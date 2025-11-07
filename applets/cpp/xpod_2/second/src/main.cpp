#include "weilsdk/error.h"
#include "weilsdk/runtime.h"
#include "weilsdk/collections/map.hpp"
#include "weilsdk/ledger.h"
#include "weilsdk/weil_contracts/fungible.h"
#include "second.hpp"
#include <iostream>
#include <string>
#include <map>
#include <vector>


extern "C" int __new(size_t len, unsigned char _id)
    __attribute__((export_name("__new")));
extern "C" void __free(size_t ptr, size_t len) __attribute__((export_name("__free")));
extern "C" void init() __attribute__((export_name("init")));
extern "C" void method_kind_data() __attribute__((export_name("method_kind_data")));
extern "C" void get_list() __attribute__((export_name("get_list")));
extern "C" void set_val() __attribute__((export_name("set_val")));


weilcontracts::Second secondContractState;

struct getListArgs{
    std::string id;
};

void to_json(nlohmann::json &j, const getListArgs &gla){
    j = nlohmann::json{
        {"id", gla.id}
    };
}

void from_json(const nlohmann::json &j, getListArgs &gla){
    j.at("id").get_to(gla.id);
}

struct setValueArgs{
    std::string id;
    uint8_t val;
};

void to_json(nlohmann::json &j, const setValueArgs &k)
{
    j = nlohmann::json{{"id", k.id}, {"val", k.val}};
}

void from_json(const nlohmann::json &j, setValueArgs &k)
{
    j.at("id").get_to(k.id);
    j.at("val").get_to(k.val);
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
  
        method_kind_mapping["get_list"]= "query";
        method_kind_mapping["set_val"]= "mutate";
        
        nlohmann::json json_object = method_kind_mapping;
        std::string serialized_string = json_object.dump();
        weilsdk::Runtime::setResult(serialized_string,0);
    }

    void init() {    
        nlohmann::json j;
        to_json(j,secondContractState);
        std::string stateString = j.dump();
  
        weilsdk::WeilValue wv;
        wv.new_with_state_and_ok_value(stateString, "Ok");
  
        weilsdk::Runtime::setStateAndResult(std::variant<weilsdk::WeilValue,std::string> {wv});
    }

    void get_list(){
        std::pair<std::string, std::string> p = weilsdk::Runtime::stateAndArgs();

        std::string raw_args = p.second;
        nlohmann::json j = nlohmann::json::parse(raw_args);
        
        if (j.is_discarded() || !j.contains("id"))
        {
            weilsdk::MethodError me = weilsdk::MethodError("get_list", "invalid_args");
            weilsdk::Runtime::setResult(weilsdk::WeilError::MethodArgumentDeserializationError(me), 1);
            return;
        }

        getListArgs args;
        args = j.get<getListArgs>();
    
        std::string stateString = p.first;
        nlohmann::json j1 = nlohmann::json::parse(stateString);

        from_json(j1,secondContractState);
        
        std::vector<uint8_t> res = secondContractState.get_list(args.id);
        nlohmann::json j2 = secondContractState;

        weilsdk::WeilValue wv;
        nlohmann::json j3 = res;
        wv.new_with_state_and_ok_value(j2.dump(),j3.dump());
        weilsdk::Runtime::setStateAndResult(std::variant<weilsdk::WeilValue,std::string> {wv});
    }

    void set_val(){
        
        std::pair<std::string, std::string> p = weilsdk::Runtime::stateAndArgs();

        std::string raw_args = p.second;
        nlohmann::json j = nlohmann::json::parse(raw_args);
        
        if (j.is_discarded() || !j.contains("id") || !j.contains("val"))
        {
            weilsdk::MethodError me = weilsdk::MethodError("set_val", "invalid_args");
            weilsdk::Runtime::setResult(weilsdk::WeilError::MethodArgumentDeserializationError(me), 1);
            return;
        }

        setValueArgs args;
        args = j.get<setValueArgs>();
    
        std::string stateString = p.first;
        nlohmann::json j1 = nlohmann::json::parse(stateString);

        from_json(j1,secondContractState);

        std::vector<uint8_t> res = secondContractState.set_val(args.id, args.val);
        
        nlohmann::json j2 = secondContractState;
        nlohmann::json j3 = res;
        weilsdk::WeilValue wv;

        wv.new_with_state_and_ok_value(j2.dump(),j3.dump());
        weilsdk::Runtime::setStateAndResult(std::variant<weilsdk::WeilValue,std::string> {wv});        
    }

}
