#define PY_SSIZE_T_CLEAN
#include <Python.h>
#include "weil_secure_wallet.h"

typedef struct {
    PyObject_HEAD
    weil_secure_wallet *wallet;
} PySecureKey;

static PyObject *raise_native(const char *error) {
    PyErr_SetString(PyExc_RuntimeError, error[0] ? error : "native secure wallet operation failed");
    return NULL;
}

static PyObject *wrap_wallet(PyTypeObject *type, weil_secure_wallet *wallet) {
    PySecureKey *self=(PySecureKey*)type->tp_alloc(type,0);
    if(!self){weil_secure_wallet_free(wallet);return NULL;}self->wallet=wallet;return(PyObject*)self;
}

static PyObject *metadata_string(const char *value) {
    if (value[0]) return PyUnicode_FromString(value);
    Py_INCREF(Py_None);
    return Py_None;
}

static PyObject *result_tuple(PyTypeObject *type,weil_secure_wallet*wallet) {
    char error[WEIL_SECURE_ERROR_SIZE]={0};weil_secure_metadata metadata;PyObject*key=NULL,*address=NULL,*org=NULL,*subgroup=NULL,*purpose=NULL,*result=NULL;
    if(weil_secure_wallet_metadata(wallet,&metadata,error)){weil_secure_wallet_free(wallet);return raise_native(error);}key=wrap_wallet(type,wallet);if(!key)return NULL;
    address=PyUnicode_FromString(metadata.address);org=metadata_string(metadata.organization);subgroup=metadata_string(metadata.subgroup);purpose=metadata_string(metadata.purpose);
    if(address&&org&&subgroup&&purpose)result=PyTuple_Pack(5,key,address,org,subgroup,purpose);
    Py_XDECREF(key);Py_XDECREF(address);Py_XDECREF(org);Py_XDECREF(subgroup);Py_XDECREF(purpose);return result;
}

static void dealloc(PySecureKey*self){weil_secure_wallet_free(self->wallet);self->wallet=NULL;Py_TYPE(self)->tp_free((PyObject*)self);}
static PyObject *close_key(PySecureKey*self,PyObject*ignored){weil_secure_wallet_free(self->wallet);self->wallet=NULL;Py_RETURN_NONE;}

static PyObject *from_fd(PyTypeObject*type,PyObject*args,PyObject*kwargs){static char*names[]={"fd","require_lock",NULL};int fd,lock=1;char error[WEIL_SECURE_ERROR_SIZE]={0};weil_secure_wallet*wallet=NULL;if(!PyArg_ParseTupleAndKeywords(args,kwargs,"i|p",names,&fd,&lock))return NULL;if(weil_secure_wallet_from_key_fd(fd,"",lock,&wallet,error))return raise_native(error);return wrap_wallet(type,wallet);}
static PyObject *from_wallet_fd(PyTypeObject*type,PyObject*args,PyObject*kwargs){static char*names[]={"fd","require_lock",NULL};int fd,lock=1;char error[WEIL_SECURE_ERROR_SIZE]={0};weil_secure_wallet*wallet=NULL;if(!PyArg_ParseTupleAndKeywords(args,kwargs,"i|p",names,&fd,&lock))return NULL;if(weil_secure_wallet_from_wallet_fd(fd,lock,&wallet,error))return raise_native(error);return result_tuple(type,wallet);}
static PyObject *from_api(PyTypeObject*type,PyObject*args,PyObject*kwargs){static char*names[]={"api_key","endpoint","verify","require_lock","include_creds",NULL};const char*api,*endpoint;int verify=1,lock=1,creds=0,rc;char error[WEIL_SECURE_ERROR_SIZE]={0};weil_secure_wallet*wallet=NULL;if(!PyArg_ParseTupleAndKeywords(args,kwargs,"ss|ppp",names,&api,&endpoint,&verify,&lock,&creds))return NULL;
    Py_BEGIN_ALLOW_THREADS rc=weil_secure_wallet_from_api(api,endpoint,verify,lock,creds,&wallet,error);Py_END_ALLOW_THREADS
    if(rc)return raise_native(error);return result_tuple(type,wallet);}
static PyObject *sign_digest(PySecureKey*self,PyObject*arg){Py_buffer digest;unsigned char signature[64];char error[WEIL_SECURE_ERROR_SIZE]={0};PyObject*result;if(PyObject_GetBuffer(arg,&digest,PyBUF_SIMPLE))return NULL;if(digest.len!=32){PyBuffer_Release(&digest);PyErr_SetString(PyExc_ValueError,"digest must be 32 bytes");return NULL;}int rc=weil_secure_wallet_sign_digest(self->wallet,digest.buf,signature,error);PyBuffer_Release(&digest);if(rc)return raise_native(error);result=PyBytes_FromStringAndSize((char*)signature,64);memset(signature,0,64);return result;}
static PyObject *public_key(PySecureKey*self,PyObject*ignored){unsigned char public[65];char error[WEIL_SECURE_ERROR_SIZE]={0};if(weil_secure_wallet_public_key(self->wallet,public,error))return raise_native(error);return PyBytes_FromStringAndSize((char*)public,65);}
static PyObject *status(PySecureKey*self,PyObject*ignored){weil_secure_status value;char error[WEIL_SECURE_ERROR_SIZE]={0};if(weil_secure_wallet_status(self->wallet,&value,error))return raise_native(error);return Py_BuildValue("{s:O,s:O,s:O,s:O}","locked",value.locked?Py_True:Py_False,"dontdump",value.dontdump?Py_True:Py_False,"guard_pages",value.guard_pages?Py_True:Py_False,"wipeonfork",value.wipeonfork?Py_True:Py_False);}

static PyMethodDef methods[]={
 {"from_fd",(PyCFunction)from_fd,METH_CLASS|METH_VARARGS|METH_KEYWORDS,NULL},{"from_wallet_fd",(PyCFunction)from_wallet_fd,METH_CLASS|METH_VARARGS|METH_KEYWORDS,NULL},{"from_api",(PyCFunction)from_api,METH_CLASS|METH_VARARGS|METH_KEYWORDS,NULL},{"sign_digest",(PyCFunction)sign_digest,METH_O,NULL},{"public_key",(PyCFunction)public_key,METH_NOARGS,NULL},{"security_status",(PyCFunction)status,METH_NOARGS,NULL},{"close",(PyCFunction)close_key,METH_NOARGS,NULL},{NULL,NULL,0,NULL}};
static PyTypeObject SecureKeyType={PyVarObject_HEAD_INIT(NULL,0).tp_name="weil_wallet._secure_wallet.SecureKey",.tp_basicsize=sizeof(PySecureKey),.tp_flags=Py_TPFLAGS_DEFAULT,.tp_new=PyType_GenericNew,.tp_dealloc=(destructor)dealloc,.tp_methods=methods};
static PyObject *harden(PyObject*m,PyObject*ignored){char error[WEIL_SECURE_ERROR_SIZE]={0};if(weil_secure_harden_process(error))return raise_native(error);Py_RETURN_NONE;}
static PyMethodDef module_methods[]={{"harden_process",harden,METH_NOARGS,NULL},{NULL,NULL,0,NULL}};
static struct PyModuleDef module={PyModuleDef_HEAD_INIT,"_secure_wallet",NULL,-1,module_methods};
PyMODINIT_FUNC PyInit__secure_wallet(void){PyObject*m;if(PyType_Ready(&SecureKeyType)<0)return NULL;m=PyModule_Create(&module);if(!m)return NULL;Py_INCREF(&SecureKeyType);if(PyModule_AddObject(m,"SecureKey",(PyObject*)&SecureKeyType)<0){Py_DECREF(&SecureKeyType);Py_DECREF(m);return NULL;}return m;}
