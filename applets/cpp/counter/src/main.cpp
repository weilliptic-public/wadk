#include "weilsdk/runtime.h"
#include "external/nlohmann.hpp"
#include "weilsdk/error.h"
#include <stdexcept>
#include "counter.hpp"

extern "C" int __new(size_t len, unsigned char _id)  __attribute__((export_name("__new")));
extern "C" void __free(size_t ptr, size_t len) __attribute__((export_name("__free")));
extern "C" void init() __attribute__((export_name("init")));
extern "C" void method_kind_data() __attribute__((export_name("method_kind_data")));
extern "C" void get_count() __attribute__((export_name("get_count")));
extern "C" void increment() __attribute__((export_name("increment")));
extern "C" void set_value() __attribute__((export_name("set_value")));

Counter smart_contract_state;

extern "C" {
    // __new method to allocate memory
    int __new(size_t len, unsigned char _id) {
            void* ptr = weilsdk::Runtime::allocate(len);
            return reinterpret_cast<int>(ptr);  // Return the pointer as an integer to track the memory location
    }

    void __free(size_t ptr, size_t len){
            weilsdk::Runtime::deallocate(ptr, len);
    }

    void init() {
            nlohmann::json j;
            to_json(j,smart_contract_state);
            std::string serializedPayload = j.dump();
 
            weilsdk::WeilValue wv;
            wv.new_with_state_and_ok_value(serializedPayload, "Ok");
            
            weilsdk::Runtime::setStateAndResult(std::variant<weilsdk::WeilValue,std::string> {wv});
    }

    void method_kind_data() {
           std::map<std::string, std::string> method_kind_mapping;
           
           method_kind_mapping["get_count"]= "query";
           method_kind_mapping["increment"]= "mutate";
           method_kind_mapping["set_value"]= "mutate";    

           nlohmann::json json_object = method_kind_mapping;

           std::string serialized_string = json_object.dump();
           weilsdk::Runtime::setResult(serialized_string,0);
    }

    void get_count() {

            std::string serializedState = weilsdk::Runtime::state();
            nlohmann::json j = nlohmann::json::parse(serializedState);
            from_json(j,smart_contract_state);

            int result = smart_contract_state.getCount();
            
            std::string serialized_result = std::to_string(result);
            weilsdk::Runtime::setResult(serialized_result, 0);
    }

    void increment() {
            std::string serializedState = weilsdk::Runtime::state();
            nlohmann::json j = nlohmann::json::parse(serializedState);
            from_json(j,smart_contract_state);
            smart_contract_state.increment();

            //TODO: ideally the increment should return the result and
            //we should not have to call getCount again
            //right now the increment is a void function
            int incremented_count = smart_contract_state.getCount();
            
            nlohmann::json j1;
            to_json(j1,smart_contract_state);
            std::string serializedPayload = j1.dump(); 

            weilsdk::WeilValue wv;
            wv.new_with_state_and_ok_value(serializedPayload,std::to_string(incremented_count));
            weilsdk::Runtime::setStateAndResult(std::variant<weilsdk::WeilValue,std::string> {wv});
    }

    void set_value() {

            std::pair<std::string,std::string> serializedStateAndArgs = weilsdk::Runtime::stateAndArgs();
            weilsdk::StateArgsValue sav;

            nlohmann::json stateJson = nlohmann::json::parse(serializedStateAndArgs.first);
            from_json(stateJson,smart_contract_state);

            nlohmann::json argsJson = nlohmann::json::parse(serializedStateAndArgs.second);
            if(argsJson.is_discarded()){
                weilsdk::MethodError me = weilsdk::MethodError("set_value", "invalid_args");
                std::string err = weilsdk::WeilError::MethodArgumentDeserializationError(me);
                weilsdk::Runtime::setStateAndResult(std::variant<weilsdk::WeilValue,std::string> {err});
                return;
            }
            setValueArgs s;
            from_json(argsJson,s);

            smart_contract_state.setValue(s.val);

            nlohmann::json j2;
            to_json(j2,smart_contract_state);
            std::string serializedSmartContractState = j2.dump(); 

            weilsdk::WeilValue wv;
            wv.new_with_state_and_ok_value(serializedSmartContractState, "Ok");
            //TODO: the "ok" needs to come as a result from smart_contract_state.setValue()
            //currently we can't make it generic as we are exprected to give a string
            //as second argument to new_with_state_and_ok_value
            weilsdk::Runtime::setStateAndResult(std::variant<weilsdk::WeilValue,std::string> {wv});
    }
}
