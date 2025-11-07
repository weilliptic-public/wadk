#include "weilsdk/error.h"
#include "weilsdk/runtime.h"
#include "weilsdk/collections/map.hpp"
#include "B.hpp"
#include <string>


extern "C" int __new(size_t len, unsigned char _id)
    __attribute__((export_name("__new")));
extern "C" void __free(size_t ptr, size_t len) __attribute__((export_name("__free")));
extern "C" void init() __attribute__((export_name("init")));
extern "C" void method_kind_data() __attribute__((export_name("method_kind_data")));
extern "C" void generate_greetings_1() __attribute__((export_name("generate_greetings_1")));
extern "C" void generate_greetings_2() __attribute__((export_name("generate_greetings_2")));
extern "C" void generate_greetings_3() __attribute__((export_name("generate_greetings_3")));

BContractState bContractState;

extern "C" {
  int __new(size_t len, unsigned char _id) {
    void *ptr = weilsdk::Runtime::allocate(len);
    return reinterpret_cast<int>(ptr);
  }
  
  void __free(size_t ptr, size_t len){
          weilsdk::Runtime::deallocate(ptr, len);
  }
  
  void init() {
    weilsdk::WeilValue wv;
    wv.new_with_state_and_ok_value("", "Ok");
    weilsdk::Runtime::setStateAndResult(std::variant<weilsdk::WeilValue,std::string> {wv});
  }

  void method_kind_data() {
      std::map<std::string, std::string> method_kind_mapping;

      method_kind_mapping["generate_greetings_1"]= "query";
      method_kind_mapping["generate_greetings_2"]= "query";
      method_kind_mapping["generate_greetings_3"]= "mutate";
      
      nlohmann::json json_object = method_kind_mapping;
      std::string serialized_string = json_object.dump();
      weilsdk::Runtime::setResult(serialized_string,0);
  }

  void generate_greetings_1(){

    std::pair<std::string, std::string> p = weilsdk::Runtime::stateAndArgs();

    std::string raw_args = p.second;
    nlohmann::json j = nlohmann::json::parse(raw_args);

    if (j.is_discarded() || !j.contains("name"))
    {
        weilsdk::MethodError me = weilsdk::MethodError("generate_greetings_1", "invalid_args");
        weilsdk::Runtime::setResult(weilsdk::WeilError::MethodArgumentDeserializationError(me), 1);
        return;
    }
    GreetingsArgs args;
    args = j.get<GreetingsArgs>();

    std::string result = bContractState.generate_greetings_1(args.name);
    weilsdk::Runtime::setResult(result,0);
  }

  void generate_greetings_2(){

    std::pair<std::string, std::string> p = weilsdk::Runtime::stateAndArgs();

    std::string raw_args = p.second;
    nlohmann::json j = nlohmann::json::parse(raw_args);

    if (j.is_discarded() || !j.contains("name"))
    {
        weilsdk::MethodError me = weilsdk::MethodError("generate_greetings_2", "invalid_args");
        weilsdk::Runtime::setResult(weilsdk::WeilError::MethodArgumentDeserializationError(me), 1);
        return;
    }
    GreetingsArgs args;
    args = j.get<GreetingsArgs>();

    std::string result = bContractState.generate_greetings_2(args.name);
    weilsdk::Runtime::setResult(result,0);
  }

  void generate_greetings_3(){

    std::pair<std::string, std::string> p = weilsdk::Runtime::stateAndArgs();

    std::string raw_args = p.second;
    nlohmann::json j = nlohmann::json::parse(raw_args);

    if (j.is_discarded() || !j.contains("name"))
    {
        weilsdk::MethodError me = weilsdk::MethodError("generate_greetings_3", "invalid_args");
        weilsdk::Runtime::setResult(weilsdk::WeilError::MethodArgumentDeserializationError(me),1);
        return;
    }
    GreetingsArgs args;
    args = j.get<GreetingsArgs>();

    std::string result = bContractState.generate_greetings_3(args.name);

    weilsdk::WeilValue wv;
    wv.new_with_state_and_ok_value("", result);
    weilsdk::Runtime::setStateAndResult(std::variant<weilsdk::WeilValue,std::string> {wv});
  }

}