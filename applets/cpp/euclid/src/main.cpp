#include "weilsdk/runtime.h"
#include "external/nlohmann.hpp"
#include "weilsdk/error.h"
#include "euclid.hpp"

extern "C" int __new(size_t len, unsigned char _id)  __attribute__((export_name("__new")));
extern "C" void init() __attribute__((export_name("init")));
extern "C" void get_size() __attribute__((export_name("get_size")));
extern "C" void add() __attribute__((export_name("add")));
extern "C" void remove_last() __attribute__((export_name("remove_last")));
extern "C" void clear() __attribute__((export_name("clear")));
extern "C" void reset() __attribute__((export_name("reset")));
extern "C" void sum_all() __attribute__((export_name("sum_all")));

Euclid euclid(1);

extern "C" {

    // __new method to allocate memory
    int __new(size_t len, unsigned char _id) {
            void* ptr = weilsdk::Runtime::allocate(len);
            return reinterpret_cast<int>(ptr);  // Return the pointer as an integer to track the memory location
    }

    void init() {
            nlohmann::json j;
            to_json(j,euclid);
            std::string serializedPayload = j.dump(); 
            weilsdk::Runtime::setState(serializedPayload);
            weilsdk::Runtime::setResult("Ok", 0);
    }

    void get_size() {

            std::string serializedState = weilsdk::Runtime::state();
            nlohmann::json j = nlohmann::json::parse(serializedState);
            from_json(j,euclid);

            int result = euclid.size();
            
            std::string serialized_result = std::to_string(result);
            weilsdk::Runtime::setResult(serialized_result, 0);
    }


    void add() {

            std::string serializedState = weilsdk::Runtime::state();
            nlohmann::json j = nlohmann::json::parse(serializedState);
            from_json(j,euclid);

            addArgs args;
            std::string raw_args = weilsdk::Runtime::args();
            nlohmann::json j1 = nlohmann::json::parse(raw_args);

            if (j1.is_discarded() || !j1.contains("elem"))
            {
                weilsdk::MethodError me = weilsdk::MethodError("elem", "invalid_args");
                weilsdk::Runtime::setResult(weilsdk::WeilError::MethodArgumentDeserializationError(me), 0);
                return;
            }
            from_json(j1,args);

            euclid.add(args.elem);

            nlohmann::json j2;
            to_json(j2,euclid);
            std::string serializedPayload = j2.dump(); 
            weilsdk::Runtime::setState(serializedPayload);

            weilsdk::Runtime::setResult("Ok",0);
    }

    void remove_last() {
            std::string serializedState = weilsdk::Runtime::state();
            nlohmann::json j = nlohmann::json::parse(serializedState);
            from_json(j,euclid);

            int x  = euclid.remove_last();

            nlohmann::json j1;
            to_json(j1,euclid);
            std::string serializedPayload = j1.dump(); 
            weilsdk::Runtime::setState(serializedPayload);

            weilsdk::Runtime::setResult(std::to_string(x),0);
    }


    void clear() {
            std::string serializedState = weilsdk::Runtime::state();
            nlohmann::json j = nlohmann::json::parse(serializedState);
            from_json(j,euclid);

            euclid.clear();

            nlohmann::json j1;
            to_json(j1,euclid);
            std::string serializedPayload = j1.dump(); 
            weilsdk::Runtime::setState(serializedPayload);

            weilsdk::Runtime::setResult("Ok",0);
    }


    void reset() {
            std::string serializedState = weilsdk::Runtime::state();
            nlohmann::json j = nlohmann::json::parse(serializedState);
            from_json(j,euclid);

     
            resetArgs args;
            std::string raw_args = weilsdk::Runtime::args();

            nlohmann::json j1 = nlohmann::json::parse(raw_args);

            if(j1.is_discarded()){
                weilsdk::MethodError me = weilsdk::MethodError("reset", "invalid_args");
                weilsdk::Runtime::setResult(weilsdk::WeilError::MethodArgumentDeserializationError(me), 0);
                return;
            }

            args = j1.get<resetArgs>();
            
            euclid.reset(args.new_size);

            nlohmann::json j2;
            to_json(j2,euclid);
            std::string serializedPayload = j1.dump(); 
            weilsdk::Runtime::setState(serializedPayload);

            weilsdk::Runtime::setResult("Ok",0); 
    }

    void sum_all() {
            std::string serializedState = weilsdk::Runtime::state();
            nlohmann::json j = nlohmann::json::parse(serializedState);
            from_json(j,euclid);

            int x  = euclid.sum_all();

            nlohmann::json j1;
            to_json(j1,euclid);
            std::string serializedPayload = j1.dump(); 
            weilsdk::Runtime::setState(serializedPayload);

            weilsdk::Runtime::setResult(std::to_string(x),0);
    }
}