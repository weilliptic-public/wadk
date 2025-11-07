#include "weilsdk/error.h"
#include "weilsdk/runtime.h"
#include "weilsdk/collections/map.hpp"
#include "weilsdk/ledger.h"
#include "weilsdk/utils.h"
#include "first.hpp"
#include <iostream>
#include <string>
#include <map>
#include <vector>


extern "C" int __new(size_t len, unsigned char _id)
    __attribute__((export_name("__new")));
extern "C" void __free(size_t ptr, size_t len) __attribute__((export_name("__free")));
extern "C" void init() __attribute__((export_name("init")));
extern "C" void method_kind_data() __attribute__((export_name("method_kind_data")));
extern "C" void set_list_in_second() __attribute__((export_name("set_list_in_second")));
extern "C" void health_check() __attribute__((export_name("health_check")));
extern "C" void counter() __attribute__((export_name("counter")));
extern "C" void set_list_in_second_callback() __attribute__((export_name("set_val_in_second_callback")));


weilcontracts::First firstContractState;

struct counterArgs{
    std::string id;
};

void to_json(nlohmann::json &j, const counterArgs &ca){
    j = nlohmann::json{
        {"id", ca.id}
    };
}

void from_json(const nlohmann::json &j, counterArgs &ca){
    j.at("id").get_to(ca.id);
}

struct setListInSecondArgs{
    std::string id;
    std::string contract_id;
    uint8_t val;
};

void to_json(nlohmann::json &j, const setListInSecondArgs &slisa){
    j = nlohmann::json{
        {"id", slisa.id},
        {"contract_id", slisa.contract_id},
        {"val", slisa.val},
    };
}

void from_json(const nlohmann::json &j, setListInSecondArgs &slisa){
    j.at("id").get_to(slisa.id);
    j.at("contract_id").get_to(slisa.contract_id);
    j.at("val").get_to(slisa.val);
}

struct setListInSecondCallbackArgs{
    std::string xpod_id;
    std::string result;
};

void to_json(nlohmann::json &j, const setListInSecondCallbackArgs &slisca){
    j = nlohmann::json{
        {"xpod_id", slisca.xpod_id},
        {"result", slisca.result}
    };
}

void from_json(const nlohmann::json &j, setListInSecondCallbackArgs &slisca){
    j.at("xpod_id").get_to(slisca.xpod_id);
    j.at("result").get_to(slisca.result);
}

extern "C" {
    int __new(size_t len, unsigned char _id) {
        void *ptr = weilsdk::Runtime::allocate(len);
        return reinterpret_cast<int>(ptr);
      }
      
      void __free(size_t ptr, size_t len){
              weilsdk::Runtime::deallocate(ptr, len);
      }
      
      void method_kind_data() {
        std::map<std::string, std::string> method_kind_mapping;
  
        method_kind_mapping["health_check"]= "query";
        method_kind_mapping["counter"]= "query";
        method_kind_mapping["set_list_in_second"]= "mutate";
        method_kind_mapping["set_list_in_second_callback"]= "mutate";

        
        nlohmann::json json_object = method_kind_mapping;
        std::string serialized_string = json_object.dump();
        weilsdk::Runtime::setResult(serialized_string,0);
    }

    void init() {    
        nlohmann::json j;
        to_json(j,firstContractState);
        std::string stateString = j.dump();
  
        weilsdk::WeilValue wv;
        wv.new_with_state_and_ok_value(stateString, "Ok");
  
        weilsdk::Runtime::setStateAndResult(std::variant<weilsdk::WeilValue,std::string> {wv});
    }

    void health_check() {    
        std::pair<std::string, std::string> p = weilsdk::Runtime::stateAndArgs();

        std::string stateString = p.first;
        nlohmann::json j1 = nlohmann::json::parse(stateString);

        from_json(j1,firstContractState);

        std::string res = firstContractState.health_check();
        nlohmann::json j2 = firstContractState;

        weilsdk::WeilValue wv;
    
        wv.new_with_state_and_ok_value(j2.dump(),res);
        weilsdk::Runtime::setStateAndResult(std::variant<weilsdk::WeilValue,std::string> {wv});     
    }

    void counter() {

        std::pair<std::string, std::string> p = weilsdk::Runtime::stateAndArgs();

        std::string stateString = p.first;
        nlohmann::json j1 = nlohmann::json::parse(stateString);

        from_json(j1,firstContractState);

        std::string raw_args = p.second;
        nlohmann::json j = nlohmann::json::parse(raw_args);
        
        if (j.is_discarded() || !j.contains("id"))
        {
            weilsdk::MethodError me = weilsdk::MethodError("counter", "invalid_args");
            weilsdk::Runtime::setResult(weilsdk::WeilError::MethodArgumentDeserializationError(me), 1);
            return;
        }

        counterArgs args;
        args = j.get<counterArgs>();

        std::pair<bool, uint32_t> res = firstContractState.counter(args.id);
        nlohmann::json j2 = firstContractState;

        if(res.first){
            //error
            weilsdk::MethodError me = weilsdk::MethodError("counter","could not get id");
            std::string err = weilsdk::WeilError::FunctionReturnedWithError(me);
            weilsdk::Runtime::setStateAndResult(std::variant<weilsdk::WeilValue,std::string> {err});
        }
        else{
            weilsdk::WeilValue wv;
            wv.new_with_state_and_ok_value(j2.dump(),std::to_string(res.second));
            weilsdk::Runtime::setStateAndResult(std::variant<weilsdk::WeilValue,std::string> {wv});
        }
    }

    void set_list_in_second(){
        std::pair<std::string, std::string> p = weilsdk::Runtime::stateAndArgs();

        std::string raw_args = p.second;
        nlohmann::json j = nlohmann::json::parse(raw_args);
        weilsdk::Runtime::debugLog("got state and args");
        if (j.is_discarded() || !j.contains("contract_id") || !j.contains("id") || !j.contains("val") )
        {
            weilsdk::MethodError me = weilsdk::MethodError("set_list_in_second", "invalid_args");
            weilsdk::Runtime::setResult(weilsdk::WeilError::MethodArgumentDeserializationError(me), 1);
            return;
        }
        setListInSecondArgs args;
        args = j.get<setListInSecondArgs>();
        std::string stateString = p.first;
        nlohmann::json j1 = nlohmann::json::parse(stateString);

        from_json(j1,firstContractState);
        firstContractState.set_list_in_second(args.contract_id, args.id, args.val);
        
        nlohmann::json j2 = firstContractState;

        weilsdk::WeilValue wv;

        wv.new_with_state_and_ok_value(j2.dump(),"Ok");
        weilsdk::Runtime::setStateAndResult(std::variant<weilsdk::WeilValue,std::string> {wv});        
        weilsdk::Runtime::debugLog("set state and result");
    }

    void set_list_in_second_callback(){
        std::pair<std::string, std::string> p = weilsdk::Runtime::stateAndArgs();

        std::string raw_args = p.second;
        nlohmann::json j = nlohmann::json::parse(raw_args);


        if (j.is_discarded() || !j.contains("result") || !j.contains("xpod_id"))
        {
            weilsdk::MethodError me = weilsdk::MethodError("set_list_in_second_callback", "invalid_args");
            weilsdk::Runtime::setResult(weilsdk::WeilError::MethodArgumentDeserializationError(me), 1);
            return;
        }

        setListInSecondCallbackArgs args;
        args = j.get<setListInSecondCallbackArgs>();

        auto deserialized_args = weilsdk::tryIntoResult<std::vector<uint8_t>>(args.result);

        struct Args {
            std::variant<std::vector<uint8_t>, weilsdk::WeilError> result;
        };

        Args parsed_args{deserialized_args};

        if(std::holds_alternative<weilsdk::WeilError>(parsed_args.result)){
            weilsdk::MethodError me = weilsdk::MethodError("get_result_from_second_callback", "invalid_result");
            weilsdk::Runtime::setResult(weilsdk::WeilError::MethodArgumentDeserializationError(me), 1);
            return;
        }

        std::vector<uint8_t> data = std::get<std::vector<uint8_t>>(parsed_args.result);
        std::string res(data.begin(), data.end());
        
        firstContractState.set_list_in_second_callback(args.xpod_id, res);

        nlohmann::json j2;
        to_json(j2,firstContractState);
        std::string stateString = j2.dump();

        weilsdk::WeilValue wv;
        wv.new_with_state_and_ok_value(stateString, "Ok");
        weilsdk::Runtime::setStateAndResult(std::variant<weilsdk::WeilValue,std::string> {wv});
    }
}
