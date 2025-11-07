#include "weilsdk/error.h"
#include "weilsdk/runtime.h"
#include "weilsdk/collections/map.hpp"
#include "weilsdk/ledger.h"
#include "weilsdk/utils.h"
#include "A.hpp"
#include <iostream>
#include <string>
#include <map>
#include <vector>
#include <variant>


extern "C" int __new(size_t len, unsigned char _id)
    __attribute__((export_name("__new")));
extern "C" void __free(size_t ptr, size_t len) __attribute__((export_name("__free")));
extern "C" void init() __attribute__((export_name("init")));
extern "C" void method_kind_data() __attribute__((export_name("method_kind_data")));
extern "C" void greetings() __attribute__((export_name("greetings")));
extern "C" void x_greetings() __attribute__((export_name("x_greetings")));
extern "C" void x_greetings_callback() __attribute__((export_name("x_greetings_callback")));


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

      method_kind_mapping["greetings"]= "query";
      method_kind_mapping["x_greetings"]= "mutate";
      method_kind_mapping["x_greetings_callback"]= "mutate";
      
      nlohmann::json json_object = method_kind_mapping;
      std::string serialized_string = json_object.dump();
      weilsdk::Runtime::setResult(serialized_string,0);
  }

  AContractState aContractState;

  void init() {
        
    nlohmann::json j;
    to_json(j,aContractState);
    std::string stateString = j.dump();

    weilsdk::WeilValue wv;
    wv.new_with_state_and_ok_value(stateString, "Ok");

    weilsdk::Runtime::setStateAndResult(std::variant<weilsdk::WeilValue,std::string> {wv});
  }

  void greetings(){
    std::pair<std::string, std::string> p = weilsdk::Runtime::stateAndArgs();

    std::string raw_args = p.second;
    nlohmann::json j = nlohmann::json::parse(raw_args);


    if (j.is_discarded() || !j.contains("name") || !j.contains("contract_addr"))
    {
        weilsdk::MethodError me = weilsdk::MethodError("greetings", "invalid_args");
        weilsdk::Runtime::setResult(weilsdk::WeilError::MethodArgumentDeserializationError(me), 1);
        return;
    }
    GreetingsArgs args;
    args = j.get<GreetingsArgs>();

    std::string stateString = p.first;
    nlohmann::json j1 = nlohmann::json::parse(stateString);

    from_json(j1,aContractState);

    std::pair<bool, std::string> result = aContractState.greetings(args.name, args.contract_addr);

    if(result.first==0){
      //no error
      nlohmann::json j2;
      to_json(j2,aContractState);
      std::string stateString = j2.dump();

      weilsdk::WeilValue wv;
      wv.new_with_state_and_ok_value(stateString, result.second);

      weilsdk::Runtime::setStateAndResult(std::variant<weilsdk::WeilValue,std::string> {wv});
    }
    else{
      weilsdk::MethodError me = weilsdk::MethodError("greetings",result.second);
      weilsdk::Runtime::setStateAndResult(weilsdk::WeilError::FunctionReturnedWithError(me));
    }
  }

  void x_greetings(){
    std::pair<std::string, std::string> p = weilsdk::Runtime::stateAndArgs();

    std::string raw_args = p.second;
    nlohmann::json j = nlohmann::json::parse(raw_args);


    if (j.is_discarded() || !j.contains("name") || !j.contains("contract_addr"))
    {
        weilsdk::MethodError me = weilsdk::MethodError("x_greetings", "invalid_args");
        weilsdk::Runtime::setResult(weilsdk::WeilError::MethodArgumentDeserializationError(me), 1);
        return;
    }
    GreetingsArgs args;
    args = j.get<GreetingsArgs>();

    std::string stateString = p.first;
    nlohmann::json j1 = nlohmann::json::parse(stateString);

    from_json(j1,aContractState);

    std::pair<bool, std::string> result = aContractState.x_greetings(args.name, args.contract_addr);

    if(result.first==0){
      //no error
      nlohmann::json j2;
      to_json(j2,aContractState);
      std::string stateString = j2.dump();

      weilsdk::WeilValue wv;
      wv.new_with_state_and_ok_value(stateString, result.second);

      weilsdk::Runtime::setStateAndResult(std::variant<weilsdk::WeilValue,std::string> {wv});
    }
    else{
      weilsdk::MethodError me = weilsdk::MethodError("x_greetings",result.second);
      weilsdk::Runtime::setStateAndResult(weilsdk::WeilError::FunctionReturnedWithError(me));
    }
  }

  void x_greetings_callback(){
    weilsdk::Runtime::debugLog("came in callback");
    std::pair<std::string, std::string> p = weilsdk::Runtime::stateAndArgs();

    std::string raw_args = p.second;
    nlohmann::json j = nlohmann::json::parse(raw_args);


    if (j.is_discarded() || !j.contains("result"))
    {
        weilsdk::MethodError me = weilsdk::MethodError("x_greetings_callback", "invalid_args");
        weilsdk::Runtime::setResult(weilsdk::WeilError::MethodArgumentDeserializationError(me), 1);
        return;
    }
    weilsdk::Runtime::debugLog("will try to parse into result");

    auto deserialized_args = weilsdk::tryIntoResult<std::string>(j["result"]);
    weilsdk::Runtime::debugLog("parsed into result");

    struct Args {
        std::variant<std::string, weilsdk::WeilError> result;
    };

    Args parsed_args{deserialized_args};

    if(std::holds_alternative<weilsdk::WeilError>(parsed_args.result)){
        weilsdk::Runtime::debugLog("holds alternative for weilerror");
        weilsdk::MethodError me = weilsdk::MethodError("x_greetings_callback", "invalid_result");
        weilsdk::Runtime::setResult(weilsdk::WeilError::MethodArgumentDeserializationError(me), 1);
        return;
    }
    weilsdk::Runtime::debugLog("result is not a weilerror");

    std::string res = std::get<std::string>(parsed_args.result);
    aContractState.x_greetings_callback(res);
    weilsdk::Runtime::debugLog("called x callback");

    nlohmann::json j2;
    to_json(j2,aContractState);
    std::string stateString = j2.dump();

    weilsdk::WeilValue wv;
    wv.new_with_state_and_ok_value(stateString, "Ok");
    weilsdk::Runtime::setStateAndResult(std::variant<weilsdk::WeilValue,std::string> {wv});
  }
}