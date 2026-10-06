/* Minimal embedded jsmn-style JSON tokenizer. Tokens reference the input and
 * never copy string contents. */
#ifndef WEIL_JSMN_H
#define WEIL_JSMN_H

typedef enum { JSMN_UNDEFINED, JSMN_OBJECT, JSMN_ARRAY, JSMN_STRING, JSMN_PRIMITIVE } jsmntype_t;
typedef struct { jsmntype_t type; int start, end, size, parent; } jsmntok_t;
typedef struct { unsigned int pos, toknext; int toksuper; } jsmn_parser;

static void jsmn_init(jsmn_parser *p) { p->pos = p->toknext = 0; p->toksuper = -1; }
static jsmntok_t *jsmn_alloc(jsmn_parser *p, jsmntok_t *t, unsigned int n) {
    if (p->toknext >= n) return 0;
    jsmntok_t *x = &t[p->toknext++]; x->start = x->end = -1; x->size = 0; x->parent = -1; x->type = JSMN_UNDEFINED; return x;
}
static int jsmn_parse_string(jsmn_parser *p, const char *s, size_t len, jsmntok_t *t, unsigned int n) {
    unsigned int start = p->pos++;
    for (; p->pos < len; p->pos++) {
        char c = s[p->pos];
        if (c == '"') { jsmntok_t *x = jsmn_alloc(p,t,n); if (!x) return -1; x->type=JSMN_STRING; x->start=(int)start+1; x->end=(int)p->pos; x->parent=p->toksuper; return 0; }
        if ((unsigned char)c < 0x20) return -2;
        if (c == '\\') { if (++p->pos >= len) return -2; c=s[p->pos]; if (c=='u') { for(int i=0;i<4;i++) { if(++p->pos>=len) return -2; char h=s[p->pos]; if(!((h>='0'&&h<='9')||(h>='a'&&h<='f')||(h>='A'&&h<='F'))) return -2; } } else if (!(c=='"'||c=='/'||c=='\\'||c=='b'||c=='f'||c=='n'||c=='r'||c=='t')) return -2; }
    }
    return -2;
}
static int jsmn_parse_primitive(jsmn_parser *p,const char*s,size_t len,jsmntok_t*t,unsigned int n) {
    unsigned int start=p->pos;
    for(;p->pos<len;p->pos++){ char c=s[p->pos]; if(c==','||c==']'||c=='}'||c==' '||c=='\t'||c=='\r'||c=='\n') break; if((unsigned char)c<0x20||c==':'||c=='"') return -2; }
    if(start==p->pos) return -2; jsmntok_t*x=jsmn_alloc(p,t,n); if(!x)return-1; x->type=JSMN_PRIMITIVE;x->start=(int)start;x->end=(int)p->pos;x->parent=p->toksuper;p->pos--;return 0;
}
static int jsmn_parse(jsmn_parser*p,const char*s,size_t len,jsmntok_t*t,unsigned int n){
    for(;p->pos<len;p->pos++){ char c=s[p->pos]; jsmntok_t*x; int r;
        switch(c){
        case '{':case '[': x=jsmn_alloc(p,t,n);if(!x)return-1;x->type=c=='{'?JSMN_OBJECT:JSMN_ARRAY;x->start=(int)p->pos;x->parent=p->toksuper;if(p->toksuper>=0)t[p->toksuper].size++;p->toksuper=(int)p->toknext-1;break;
        case '}':case ']': { jsmntype_t ty=c=='}'?JSMN_OBJECT:JSMN_ARRAY;int i;for(i=(int)p->toknext-1;i>=0;i--)if(t[i].start!=-1&&t[i].end==-1){if(t[i].type!=ty)return-2;t[i].end=(int)p->pos+1;p->toksuper=t[i].parent;break;}if(i<0)return-2;break; }
        case '"': r=jsmn_parse_string(p,s,len,t,n);if(r<0)return r;if(p->toksuper>=0)t[p->toksuper].size++;break;
        case ' ':case '\t':case '\r':case '\n':case ':':case ',': break;
        default:r=jsmn_parse_primitive(p,s,len,t,n);if(r<0)return r;if(p->toksuper>=0)t[p->toksuper].size++;break;
        }
    }
    for(unsigned int i=0;i<p->toknext;i++)if(t[i].start!=-1&&t[i].end==-1)return-2;
    return (int)p->toknext;
}
#endif
