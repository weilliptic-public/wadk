#define _GNU_SOURCE
#include "weil_secure_wallet.h"
#include "jsmn.h"

#include <curl/curl.h>
#include <errno.h>
#include <fcntl.h>
#include <openssl/bn.h>
#include <openssl/ec.h>
#include <openssl/ecdsa.h>
#include <openssl/hmac.h>
#include <openssl/obj_mac.h>
#include <openssl/sha.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#ifdef __linux__
#include <sys/prctl.h>
#else
#define MADV_DONTDUMP 0
#define MADV_WIPEONFORK 0
#define PR_SET_DUMPABLE 0
#define madvise(address, length, advice) (0)
#define prctl(option, value, a, b, c) (0)
#endif
#include <sys/resource.h>
#include <sys/stat.h>
#include <unistd.h>

typedef struct {
    void *mapping;
    size_t mapping_len;
    unsigned char *data;
    size_t data_len;
    size_t content_len;  /* bytes of live content in data (<= data_len) */
    int locked;
    int wipeonfork;
} secure_region;

struct weil_secure_wallet {
    secure_region secret;       /* derived key in secure memory */
    weil_secure_metadata metadata;
    int closed;
};

static void set_error(char *out, const char *format, ...) {
    va_list args;
    if (!out) return;
    va_start(args, format);
    vsnprintf(out, WEIL_SECURE_ERROR_SIZE, format, args);
    va_end(args);
}

static void wipe(void *ptr, size_t length) {
#if defined(__GLIBC__) || defined(__FreeBSD__)
    explicit_bzero(ptr, length);
#else
    volatile unsigned char *p = ptr;
    while (length--) *p++ = 0;
#endif
}

static int region_create(secure_region *region, size_t needed, int require_lock,
                         char *error) {
    long page_size = sysconf(_SC_PAGESIZE);
    size_t pages;
    memset(region, 0, sizeof(*region));
    if (page_size < 128) { set_error(error, "invalid page size"); return -1; }
    pages = (needed + (size_t)page_size - 1) / (size_t)page_size;
    if (!pages) pages = 1;
    region->data_len = pages * (size_t)page_size;
    region->mapping_len = region->data_len + 2 * (size_t)page_size;
    region->mapping = mmap(NULL, region->mapping_len, PROT_NONE,
                           MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    if (region->mapping == MAP_FAILED) { region->mapping = NULL; set_error(error, "mmap: %s", strerror(errno)); return -1; }
    region->data = (unsigned char *)region->mapping + page_size;
    if (mprotect(region->data, region->data_len, PROT_READ | PROT_WRITE)) {
        set_error(error, "mprotect: %s", strerror(errno));
        munmap(region->mapping, region->mapping_len); memset(region, 0, sizeof(*region)); return -1;
    }
    if (!mlock(region->data, region->data_len)) region->locked = 1;
    else if (require_lock) { set_error(error, "mlock: %s (raise RLIMIT_MEMLOCK with ulimit -l or LimitMEMLOCK=)", strerror(errno)); munmap(region->mapping, region->mapping_len); memset(region, 0, sizeof(*region)); return -1; }
    if (madvise(region->data, region->data_len, MADV_DONTDUMP)) { set_error(error, "MADV_DONTDUMP: %s", strerror(errno)); return -1; }
#ifdef MADV_WIPEONFORK
    if (!madvise(region->data, region->data_len, MADV_WIPEONFORK)) region->wipeonfork = 1;
    else if (errno != EINVAL) { set_error(error, "MADV_WIPEONFORK: %s", strerror(errno)); return -1; }
#endif
    return 0;
}

static void region_destroy(secure_region *region) {
    if (region->mapping) {
        wipe(region->data, region->data_len);
        if (region->locked) munlock(region->data, region->data_len);
        munmap(region->mapping, region->mapping_len);
    }
    memset(region, 0, sizeof(*region));
}

static EC_KEY *ec_from_secret(const unsigned char secret[32]) {
    EC_KEY *ec = NULL; BIGNUM *private = NULL; EC_POINT *public = NULL;
    const EC_GROUP *group;
    ec = EC_KEY_new_by_curve_name(NID_secp256k1); if (!ec) goto fail;
    group = EC_KEY_get0_group(ec); private = BN_bin2bn(secret, 32, NULL);
    if (!private || BN_is_zero(private) || BN_is_negative(private)) goto fail;
    public = EC_POINT_new(group);
    if (!public || !EC_POINT_mul(group, public, private, NULL, NULL, NULL)) goto fail;
    if (!EC_KEY_set_private_key(ec, private) || !EC_KEY_set_public_key(ec, public) || !EC_KEY_check_key(ec)) goto fail;
    BN_clear_free(private); EC_POINT_clear_free(public); return ec;
fail:
    BN_clear_free(private); EC_POINT_clear_free(public); EC_KEY_free(ec); return NULL;
}

static int wallet_create(const unsigned char *data, size_t size, const weil_secure_metadata *metadata,
                         int require_lock, weil_secure_wallet **out, char *error) {
    EC_KEY *check = ec_from_secret(data);
    if (!check) { set_error(error, "invalid secp256k1 private key"); return -1; }
    EC_KEY_free(check);
    if (size != 32) { set_error(error, "invalid secret key size"); return -1; }
    weil_secure_wallet *wallet = calloc(1, sizeof(*wallet));
    if (!wallet) { set_error(error, "out of memory"); return -1; }
    if (region_create(&wallet->secret, size, require_lock, error)) { region_destroy(&wallet->secret); free(wallet); return -1; }
    memcpy(wallet->secret.data, data, size);
    wallet->secret.content_len = size;
    if (metadata) memcpy(&wallet->metadata, metadata, sizeof(*metadata));
    *out = wallet; return 0;
}

static int hex_value(unsigned char c) {
    if (c >= '0' && c <= '9') return c - '0';
    if (c >= 'a' && c <= 'f') return c - 'a' + 10;
    if (c >= 'A' && c <= 'F') return c - 'A' + 10;
    return -1;
}

static int token_equals(const char *json, const jsmntok_t *token, const char *value) {
    size_t length = strlen(value);
    return token->type == JSMN_STRING && (size_t)(token->end-token->start) == length &&
           !memcmp(json+token->start, value, length);
}

static int object_get(const char *json, jsmntok_t *tokens, int count, int object, const char *key) {
    if (object < 0 || object >= count || tokens[object].type != JSMN_OBJECT) return -1;
    for (int i=object+1; i+1<count && tokens[i].start<tokens[object].end; i++)
        if (tokens[i].parent == object && token_equals(json, &tokens[i], key)) return i+1;
    return -1;
}

static int array_at(jsmntok_t *tokens, int count, int array, int index) {
    int found=0;
    if (array<0 || array>=count || tokens[array].type!=JSMN_ARRAY) return -1;
    for (int i=array+1;i<count&&tokens[i].start<tokens[array].end;i++)
        if(tokens[i].parent==array&&found++==index)return i;
    return -1;
}

static int token_integer(const char *json,const jsmntok_t*token,int*out) {
    long value=0;if(!token||token->type!=JSMN_PRIMITIVE||token->start==token->end)return-1;
    for(int i=token->start;i<token->end;i++){if(json[i]<'0'||json[i]>'9')return-1;value=value*10+json[i]-'0';if(value>0x7fffffff)return-1;}*out=(int)value;return 0;
}

static int token_copy(const char *json,const jsmntok_t*token,char*out,size_t capacity) {
    size_t length;if(!token||token->type!=JSMN_STRING)return-1;length=(size_t)(token->end-token->start);
    if(length>=capacity)return-1;memcpy(out,json+token->start,length);out[length]=0;return 0;
}

static int decode_hex_token(const char*json,const jsmntok_t*token,unsigned char out[32]) {
    if(!token||token->type!=JSMN_STRING||token->end-token->start!=64)return-1;
    for(int i=0;i<32;i++){int hi=hex_value(json[token->start+i*2]),lo=hex_value(json[token->start+i*2+1]);if(hi<0||lo<0){wipe(out,32);return-1;}out[i]=(unsigned char)((hi<<4)|lo);}return 0;
}

static int decode_xprv(const char*s,size_t length,unsigned char key[32],unsigned char chain[32]) {
    static const char alphabet[]="123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    unsigned char raw[82]={0},hash[32],hash2[32];size_t used=1,zeros=0;int result=-1;
    for(size_t i=0;i<length;i++){const char*p=strchr(alphabet,s[i]);unsigned carry;if(!p)goto done;carry=(unsigned)(p-alphabet);for(size_t j=0;j<used;j++){carry+=58U*raw[j];raw[j]=(unsigned char)carry;carry>>=8;}while(carry){if(used>=sizeof(raw))goto done;raw[used++]=(unsigned char)carry;carry>>=8;}}
    while(zeros<length&&s[zeros]=='1')zeros++;if(used+zeros!=82)goto done;
    for(size_t i=0;i<41;i++){unsigned char c=raw[i];raw[i]=raw[81-i];raw[81-i]=c;}
    SHA256(raw,78,hash);SHA256(hash,32,hash2);if(memcmp(hash2,raw+78,4)||raw[45])goto done;
    memcpy(chain,raw+13,32);memcpy(key,raw+46,32);result=0;
done:wipe(raw,sizeof(raw));wipe(hash,sizeof(hash));wipe(hash2,sizeof(hash2));return result;
}

static int derive_child(const unsigned char parent[32],const unsigned char chain[32],unsigned index,int hardened,unsigned char child[32],unsigned char child_chain[32]) {
    unsigned char data[37],digest[64],pub[33];unsigned digest_len=0;EC_KEY*ec=NULL;BIGNUM*a=NULL,*b=NULL,*order=NULL;BN_CTX*ctx=NULL;int result=-1;
    if(hardened){data[0]=0;memcpy(data+1,parent,32);index|=0x80000000U;}else{ec=ec_from_secret(parent);if(!ec)goto done;if(EC_POINT_point2oct(EC_KEY_get0_group(ec),EC_KEY_get0_public_key(ec),POINT_CONVERSION_COMPRESSED,pub,33,NULL)!=33)goto done;memcpy(data,pub,33);}
    data[33]=(unsigned char)(index>>24);data[34]=(unsigned char)(index>>16);data[35]=(unsigned char)(index>>8);data[36]=(unsigned char)index;
    if(!HMAC(EVP_sha512(),chain,32,data,37,digest,&digest_len)||digest_len!=64)goto done;
    if(!ec)ec=ec_from_secret(parent);a=BN_bin2bn(digest,32,NULL);b=BN_bin2bn(parent,32,NULL);order=BN_new();ctx=BN_CTX_new();
    if(!ec||!a||!b||!order||!ctx||!EC_GROUP_get_order(EC_KEY_get0_group(ec),order,ctx)||!BN_mod_add(a,a,b,order,ctx)||BN_is_zero(a)||BN_bn2binpad(a,child,32)!=32)goto done;
    memcpy(child_chain,digest+32,32);result=0;
done:wipe(data,37);wipe(digest,64);wipe(pub,33);BN_clear_free(a);BN_clear_free(b);BN_clear_free(order);BN_CTX_free(ctx);EC_KEY_free(ec);return result;
}

static int public_matches(const unsigned char key[32],const char*json,const jsmntok_t*token) {
    static const char hex[]="0123456789abcdef";EC_KEY*ec=ec_from_secret(key);unsigned char pub[33];int result=0;
    if(!ec||!token||token->type!=JSMN_STRING||token->end-token->start!=66)goto done;
    if(EC_POINT_point2oct(EC_KEY_get0_group(ec),EC_KEY_get0_public_key(ec),POINT_CONVERSION_COMPRESSED,pub,33,NULL)!=33)goto done;
    result=1;for(int i=0;i<33;i++){char a=json[token->start+i*2],b=json[token->start+i*2+1];if(a>='A'&&a<='F')a+=32;if(b>='A'&&b<='F')b+=32;if(a!=hex[pub[i]>>4]||b!=hex[pub[i]&15]){result=0;break;}}
done:wipe(pub,33);EC_KEY_free(ec);return result;
}

static int derive_selected(const char*json,jsmntok_t*tokens,int count,int root,int entry,unsigned char out[32]) {
    int xprv=object_get(json,tokens,count,root,"xprv"),derived=object_get(json,tokens,count,root,"derived_accounts"),first,index_token,index;
    unsigned char master[32],chain[32],account[32],account_chain[32],next[32],next_chain[32];int result=-1;
    if(xprv<0||decode_xprv(json+tokens[xprv].start,(size_t)(tokens[xprv].end-tokens[xprv].start),master,chain))goto done;
    memcpy(account,master,32);memcpy(account_chain,chain,32);first=array_at(tokens,count,derived,0);
    if(first>=0){int first_index,fi=object_get(json,tokens,count,first,"index"),fp=object_get(json,tokens,count,first,"public_key");if(fi<0||token_integer(json,&tokens[fi],&first_index)||derive_child(master,chain,(unsigned)first_index,0,next,next_chain))goto done;
        if(!public_matches(next,json,fp>=0?&tokens[fp]:NULL)){unsigned path[]={44,9345,0,0};int hard[]={1,1,1,0};for(int i=0;i<4;i++){if(derive_child(account,account_chain,path[i],hard[i],next,next_chain))goto done;memcpy(account,next,32);memcpy(account_chain,next_chain,32);}}}
    index_token=object_get(json,tokens,count,entry,"index");if(index_token<0||token_integer(json,&tokens[index_token],&index)||derive_child(account,account_chain,(unsigned)index,0,out,next_chain))goto done;result=0;
done:wipe(master,32);wipe(chain,32);wipe(account,32);wipe(account_chain,32);wipe(next,32);wipe(next_chain,32);return result;
}

static int parse_wallet(const unsigned char*document,size_t length,int require_lock,weil_secure_wallet**out,char*error) {
    const char*json=(const char*)document;jsmntok_t*tokens=NULL;jsmn_parser parser;int count,selected,index=0,external=0,accounts,entry,key_token,address_token;unsigned char secret[32];weil_secure_metadata metadata={0};int result=-1;
    tokens=calloc(length+1,sizeof(*tokens));if(!tokens){set_error(error,"out of memory");goto done;}jsmn_init(&parser);count=jsmn_parse(&parser,json,length,tokens,(unsigned)length+1);
    if(count<1||tokens[0].type!=JSMN_OBJECT){set_error(error,"invalid wallet JSON");goto done;}
    {int type=object_get(json,tokens,count,0,"type");if(type<0||!token_equals(json,&tokens[type],"wallet")){set_error(error,"expected wallet type");goto done;}}
    selected=object_get(json,tokens,count,0,"selected_account");if(selected>=0){int kind=object_get(json,tokens,count,selected,"type"),idx=object_get(json,tokens,count,selected,"index");if(kind>=0&&token_equals(json,&tokens[kind],"external"))external=1;if(idx>=0&&token_integer(json,&tokens[idx],&index)){set_error(error,"invalid selected account");goto done;}}
    accounts=object_get(json,tokens,count,0,external?"external_accounts":"derived_accounts");entry=array_at(tokens,count,accounts,index);if(entry<0){set_error(error,"selected account missing");goto done;}
    if(external){key_token=object_get(json,tokens,count,entry,"secret_key");if(key_token<0||decode_hex_token(json,&tokens[key_token],secret)){set_error(error,"invalid external key");goto done;}}
    else if(derive_selected(json,tokens,count,0,entry,secret)){set_error(error,"failed to derive selected key");goto done;}
    address_token=object_get(json,tokens,count,entry,"account_address");if(address_token<0||token_copy(json,&tokens[address_token],metadata.address,sizeof(metadata.address))){set_error(error,"invalid account address");goto done;}
    {int orgs=object_get(json,tokens,count,entry,"orgs"),active=object_get(json,tokens,count,entry,"active_org"),org_index=0,org_entry=-1;if(active>=0)token_integer(json,&tokens[active],&org_index);if(orgs>=0)org_entry=array_at(tokens,count,orgs,org_index);
        if(org_entry<0){orgs=object_get(json,tokens,count,0,"orgs");active=object_get(json,tokens,count,0,"active_org");org_index=0;if(active>=0)token_integer(json,&tokens[active],&org_index);if(orgs>=0)org_entry=array_at(tokens,count,orgs,org_index);}
        if(org_entry>=0){int o=object_get(json,tokens,count,org_entry,"org"),s=object_get(json,tokens,count,org_entry,"subgroup");if(o>=0)token_copy(json,&tokens[o],metadata.organization,sizeof(metadata.organization));if(s>=0)token_copy(json,&tokens[s],metadata.subgroup,sizeof(metadata.subgroup));}
        else{int old=object_get(json,tokens,count,0,"org");if(old>=0&&tokens[old].type==JSMN_OBJECT){int o=object_get(json,tokens,count,old,"name"),s=object_get(json,tokens,count,old,"subgroup"),p=object_get(json,tokens,count,old,"purpose");if(o>=0)token_copy(json,&tokens[o],metadata.organization,sizeof(metadata.organization));if(s>=0)token_copy(json,&tokens[s],metadata.subgroup,sizeof(metadata.subgroup));if(p>=0)token_copy(json,&tokens[p],metadata.purpose,sizeof(metadata.purpose));}}}
    result=wallet_create(secret,32,&metadata,require_lock,out,error);
done:wipe(secret,32);wipe(&metadata,sizeof(metadata));free(tokens);return result;
}

WEIL_SECURE_API int weil_secure_harden_process(char error[WEIL_SECURE_ERROR_SIZE]) {
    struct rlimit limit={0,0};if(setrlimit(RLIMIT_CORE,&limit)){set_error(error,"setrlimit: %s",strerror(errno));return-1;}if(prctl(PR_SET_DUMPABLE,0,0,0,0)){set_error(error,"prctl: %s",strerror(errno));return-1;}return 0;
}

WEIL_SECURE_API int weil_secure_wallet_from_key_fd(int fd,const char*address,int require_lock,weil_secure_wallet**out,char*error) {
    secure_region input={0};ssize_t total=0;unsigned char secret[32];weil_secure_metadata metadata={0};int result=-1;
    if(!address||strlen(address)>=sizeof(metadata.address)){set_error(error,"invalid address");return-1;}strcpy(metadata.address,address);
    if(region_create(&input,65,require_lock,error))goto done;while(total<65){ssize_t n=read(fd,input.data+total,(size_t)(65-total));if(n<0&&errno==EINTR)continue;if(n<=0)break;total+=n;}while(total>0&&strchr("\n\r \t",input.data[total-1]))total--;
    if(total!=64){set_error(error,"key file must contain 64 hexadecimal characters");goto done;}for(int i=0;i<32;i++){int h=hex_value(input.data[i*2]),l=hex_value(input.data[i*2+1]);if(h<0||l<0){set_error(error,"invalid hexadecimal key");goto done;}secret[i]=(unsigned char)((h<<4)|l);}result=wallet_create(secret,32,&metadata,require_lock,out,error);
done:wipe(secret,32);wipe(&metadata,sizeof(metadata));region_destroy(&input);return result;
}

WEIL_SECURE_API int weil_secure_wallet_from_wallet_fd(int fd,int require_lock,weil_secure_wallet**out,char*error) {
    struct stat status;secure_region input={0};ssize_t total=0;int result=-1;if(fstat(fd,&status)){set_error(error,"fstat: %s",strerror(errno));return-1;}if(status.st_size<=0||status.st_size>1024*1024){set_error(error,"wallet size must be 1 byte to 1 MiB");return-1;}
    if(region_create(&input,(size_t)status.st_size,require_lock,error))goto done;while(total<status.st_size){ssize_t n=read(fd,input.data+total,(size_t)(status.st_size-total));if(n<0&&errno==EINTR)continue;if(n<=0)break;total+=n;}if(total!=status.st_size){set_error(error,"wallet changed while reading");goto done;}result=parse_wallet(input.data,(size_t)total,require_lock,out,error);
done:region_destroy(&input);return result;
}

typedef struct{unsigned char*data;size_t length,capacity;}curl_buffer;
static size_t receive(char*data,size_t size,size_t count,void*context){curl_buffer*b=context;size_t n;if(size&&count>SIZE_MAX/size)return 0;n=size*count;if(n>b->capacity-b->length)return 0;memcpy(b->data+b->length,data,n);b->length+=n;return n;}
static int append_json(unsigned char*out,size_t cap,size_t*used,const char*s){for(;*s;s++){unsigned char c=*s;const char*e=NULL;if(c=='"')e="\\\"";else if(c=='\\')e="\\\\";else if(c=='\n')e="\\n";else if(c=='\r')e="\\r";else if(c=='\t')e="\\t";if(e){for(;*e;e++){if(*used>=cap)return-1;out[(*used)++]=*e;}}else{if(c<0x20||*used>=cap)return-1;out[(*used)++]=c;}}return 0;}
static int field(unsigned char*out,size_t cap,size_t*used,const char*name,const char*value,int comma){if(*used+5>=cap)return-1;if(comma)out[(*used)++]=',';out[(*used)++]='"';if(append_json(out,cap,used,name))return-1;out[(*used)++]='"';out[(*used)++]=':';out[(*used)++]='"';if(append_json(out,cap,used,value)||*used>=cap)return-1;out[(*used)++]='"';return 0;}
static int unwrap(unsigned char*data,size_t*length){size_t start=0,end=*length,w=0;while(start<end&&strchr(" \n\r\t",data[start]))start++;while(end>start&&strchr(" \n\r\t",data[end-1]))end--;if(start<end&&data[start]=='{'){memmove(data,data+start,end-start);*length=end-start;return 0;}if(end-start<2||data[start]!='"'||data[end-1]!='"')return-1;for(size_t r=start+1;r<end-1;r++){unsigned char c=data[r];if(c!='\\'){data[w++]=c;continue;}if(++r>=end-1)return-1;c=data[r];if(c=='"'||c=='\\'||c=='/')data[w++]=c;else if(c=='b')data[w++]='\b';else if(c=='f')data[w++]='\f';else if(c=='n')data[w++]='\n';else if(c=='r')data[w++]='\r';else if(c=='t')data[w++]='\t';else return-1;}*length=w;return 0;}

WEIL_SECURE_API int weil_secure_wallet_from_api(const char*api_key,const char*endpoint,int verify,int require_lock,int include_creds,weil_secure_wallet**out,char*error){
    secure_region request={0},response={0};size_t used=0;CURL*curl=NULL;CURLcode code;struct curl_slist*headers=NULL;curl_buffer buffer;long status=0;int result=-1;const char*names[]={"AWS_ACCESS_KEY_ID","AWS_SECRET_ACCESS_KEY","AWS_REGION","AWS_BUCKET_NAME"},*keys[]={"access_key_id","secret_access_key","region","bucket_name"};
    if(!api_key||!endpoint||strncmp(endpoint,"https://",8)){set_error(error,"HTTPS endpoint and API key required");return-1;}if(region_create(&request,16384,require_lock,error)||region_create(&response,1024*1024,require_lock,error))goto done;
    request.data[used++]='{';if(field(request.data,request.data_len,&used,"api_key",api_key,0))goto large;request.data[used++]=',';request.data[used++]='"';memcpy(request.data+used,"unmasked",8);used+=8;request.data[used++]='"';request.data[used++]=':';request.data[used++]='t';request.data[used++]='r';request.data[used++]='u';request.data[used++]='e';if(include_creds){int any=0,comma=0;for(int i=0;i<4;i++)if(getenv(names[i])&&*getenv(names[i]))any=1;if(any){memcpy(request.data+used,",\"credentials\":{",16);used+=16;for(int i=0;i<4;i++){const char*v=getenv(names[i]);if(v&&*v){size_t vl=strlen(v);if(field(request.data,request.data_len,&used,keys[i],v,comma++))goto large;memset((void*)v,0,vl);}}request.data[used++]='}';}for(int i=0;i<4;i++)unsetenv(names[i]);}request.data[used++]='}';
    if(curl_global_init(CURL_GLOBAL_DEFAULT)!=CURLE_OK){set_error(error,"curl initialization failed");goto done;}curl=curl_easy_init();if(!curl){set_error(error,"curl initialization failed");goto done;}headers=curl_slist_append(headers,"Content-Type: application/json");headers=curl_slist_append(headers,"Accept: application/json");buffer=(curl_buffer){response.data,0,response.data_len};
    curl_easy_setopt(curl,CURLOPT_URL,endpoint);curl_easy_setopt(curl,CURLOPT_POST,1L);curl_easy_setopt(curl,CURLOPT_POSTFIELDS,request.data);curl_easy_setopt(curl,CURLOPT_POSTFIELDSIZE,(long)used);curl_easy_setopt(curl,CURLOPT_HTTPHEADER,headers);curl_easy_setopt(curl,CURLOPT_WRITEFUNCTION,receive);curl_easy_setopt(curl,CURLOPT_WRITEDATA,&buffer);curl_easy_setopt(curl,CURLOPT_FOLLOWLOCATION,0L);curl_easy_setopt(curl,CURLOPT_PROTOCOLS,CURLPROTO_HTTPS);curl_easy_setopt(curl,CURLOPT_CONNECTTIMEOUT,15L);curl_easy_setopt(curl,CURLOPT_TIMEOUT,60L);curl_easy_setopt(curl,CURLOPT_SSL_VERIFYPEER,verify?1L:0L);curl_easy_setopt(curl,CURLOPT_SSL_VERIFYHOST,verify?2L:0L);curl_easy_setopt(curl,CURLOPT_NOSIGNAL,1L);
    code=curl_easy_perform(curl);if(code!=CURLE_OK){set_error(error,"wallet request: %s",curl_easy_strerror(code));goto done;}curl_easy_getinfo(curl,CURLINFO_RESPONSE_CODE,&status);if(status!=200){size_t preview=buffer.length<200?buffer.length:200;set_error(error,"wallet request HTTP %ld: %.*s",status,(int)preview,(const char*)response.data);goto done;}if(unwrap(response.data,&buffer.length)){set_error(error,"invalid wallet response");goto done;}
    result=parse_wallet(response.data,(size_t)buffer.length,require_lock,out,error);
    goto done;
large:set_error(error,"credentials too large");
done:curl_slist_free_all(headers);if(curl)curl_easy_cleanup(curl);region_destroy(&request);region_destroy(&response);return result;
}

WEIL_SECURE_API int weil_secure_wallet_sign_digest(weil_secure_wallet*wallet,const uint8_t digest[32],uint8_t output[64],char*error){EC_KEY*ec=NULL;ECDSA_SIG*signature=NULL;const BIGNUM*r,*s;BIGNUM*order=NULL,*half=NULL,*normalized=NULL;int result=-1;if(!wallet||wallet->closed){set_error(error,"wallet closed");return-1;}ec=ec_from_secret(wallet->secret.data);signature=ec?ECDSA_do_sign(digest,32,ec):NULL;if(!signature)goto fail;ECDSA_SIG_get0(signature,&r,&s);order=BN_new();half=BN_new();if(!order||!half||!EC_GROUP_get_order(EC_KEY_get0_group(ec),order,NULL)||!BN_rshift1(half,order))goto fail;if(BN_cmp(s,half)>0){normalized=BN_new();if(!normalized||!BN_sub(normalized,order,s))goto fail;s=normalized;}if(BN_bn2binpad(r,output,32)!=32||BN_bn2binpad(s,output+32,32)!=32)goto fail;result=0;goto done;fail:set_error(error,"secp256k1 signing failed");done:BN_clear_free(normalized);BN_clear_free(half);BN_clear_free(order);ECDSA_SIG_free(signature);EC_KEY_free(ec);return result;}
WEIL_SECURE_API int weil_secure_wallet_public_key(weil_secure_wallet*wallet,uint8_t output[65],char*error){EC_KEY*ec;if(!wallet||wallet->closed){set_error(error,"wallet closed");return-1;}ec=ec_from_secret(wallet->secret.data);if(!ec||EC_POINT_point2oct(EC_KEY_get0_group(ec),EC_KEY_get0_public_key(ec),POINT_CONVERSION_UNCOMPRESSED,output,65,NULL)!=65){set_error(error,"public key failed");EC_KEY_free(ec);return-1;}EC_KEY_free(ec);return 0;}
WEIL_SECURE_API int weil_secure_wallet_status(weil_secure_wallet*wallet,weil_secure_status*status,char*error){if(!wallet||wallet->closed||!status){set_error(error,"wallet closed");return-1;}*status=(weil_secure_status){wallet->secret.locked,1,1,wallet->secret.wipeonfork};return 0;}
WEIL_SECURE_API int weil_secure_wallet_metadata(weil_secure_wallet*wallet,weil_secure_metadata*metadata,char*error){if(!wallet||wallet->closed||!metadata){set_error(error,"wallet closed");return-1;}memcpy(metadata,&wallet->metadata,sizeof(*metadata));return 0;}
WEIL_SECURE_API void weil_secure_wallet_free(weil_secure_wallet*wallet){if(!wallet)return;wallet->closed=1;region_destroy(&wallet->secret);wipe(&wallet->metadata,sizeof(wallet->metadata));wipe(wallet,sizeof(*wallet));free(wallet);}
WEIL_SECURE_API unsigned int weil_secure_wallet_abi_version(void){return 1;}
