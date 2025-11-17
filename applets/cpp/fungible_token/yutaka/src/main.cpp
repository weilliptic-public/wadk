#include "weilsdk/error.h"
#include "weilsdk/runtime.h"
#include "weilsdk/collections/map.hpp"
#include "weilsdk/ledger.h"
#include "weilsdk/weil_contracts/fungible.h"
#include "yutaka.hpp"
#include <iostream>
#include <string>
#include <map>
#include <vector>


extern "C" int __new(size_t len, unsigned char _id)
    __attribute__((export_name("__new")));
extern "C" void __free(size_t ptr, size_t len) __attribute__((export_name("__free")));
extern "C" void init() __attribute__((export_name("init")));
extern "C" void method_kind_data() __attribute__((export_name("method_kind_data")));
extern "C" void name() __attribute__((export_name("name")));
extern "C" void symbol() __attribute__((export_name("symbol")));
extern "C" void decimals() __attribute__((export_name("decimals")));
extern "C" void details() __attribute__((export_name("details")));
extern "C" void total_supply() __attribute__((export_name("total_supply")));
extern "C" void balance_for() __attribute__((export_name("balance_for")));
extern "C" void transfer() __attribute__((export_name("transfer")));
extern "C" void approve() __attribute__((export_name("approve")));
extern "C" void transfer_from() __attribute__((export_name("transfer_from")));
extern "C" void allowance() __attribute__((export_name("allowance")));


  // --------------------------------------- ****** ------------------------------------------ //

weilcontracts::Yutaka yutaka_instance;

struct balanceForArgs{
  std::string addr;
};

void to_json(nlohmann::json &j, const balanceForArgs &k)
{
    j = nlohmann::json{{"addr", k.addr}};
}

void from_json(const nlohmann::json &j, balanceForArgs &k)
{
    j.at("addr").get_to(k.addr);
}


struct transferArgs{
  std::string to_addr;
  uint64_t amount;
};

void to_json(nlohmann::json &j, const transferArgs &k)
{
    j = nlohmann::json{{"to_addr", k.to_addr}, {"amount", k.amount}};
}

void from_json(const nlohmann::json &j, transferArgs &k)
{
    j.at("to_addr").get_to(k.to_addr);
    j.at("amount").get_to(k.amount);
}

struct approveArgs{
  std::string spender;
  uint64_t amount;
};

void to_json(nlohmann::json &j, const approveArgs &k)
{
    j = nlohmann::json{{"spender", k.spender}, {"amount", k.amount}};
}

void from_json(const nlohmann::json &j, approveArgs &k)
{
    j.at("spender").get_to(k.spender);
    j.at("amount").get_to(k.amount);
}


struct transferFromArgs{
  std::string from_addr;
  std::string to_addr;
  uint64_t amount;
};

inline void to_json(nlohmann::json &j, const transferFromArgs &k)
{
    j = nlohmann::json{{"from_addr", k.from_addr}, {"to_addr", k.to_addr}, {"amount", k.amount}};
}

inline void from_json(const nlohmann::json &j, transferFromArgs &k)
{
    j.at("from_addr").get_to(k.from_addr);
    j.at("to_addr").get_to(k.to_addr);
    j.at("amount").get_to(k.amount);
}

struct allowanceForArgs{
  std::string owner;
  std::string spender;
};

void to_json(nlohmann::json &j, const allowanceForArgs &k)
{
    j = nlohmann::json{{"spender", k.spender}, {"owner", k.owner}};
}

void from_json(const nlohmann::json &j, allowanceForArgs &k)
{
    j.at("spender").get_to(k.spender);
    j.at("owner").get_to(k.owner);
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

      method_kind_mapping["name"]= "query";
      method_kind_mapping["symbol"]= "query";
      method_kind_mapping["decimals"]= "query";
      method_kind_mapping["details"]= "query";
      method_kind_mapping["total_supply"]= "query";
      method_kind_mapping["balance_for"]= "query";
      method_kind_mapping["transfer"]= "mutate";
      method_kind_mapping["approve"]= "mutate";
      method_kind_mapping["transfer_from"]= "mutate";
      method_kind_mapping["allowance"]= "query";

      nlohmann::json json_object = method_kind_mapping;
      std::string serialized_string = json_object.dump();
      weilsdk::Runtime::setResult(serialized_string,0);
  }

  void init() {
    weilcontracts::FungibleToken ft("Yutaka", "YTK");
    weilcontracts::Yutaka yutaka_instance(ft);

    uint64_t total_supply = 100000000000;
    std::pair<bool, std::string> result = yutaka_instance.inner.mint(total_supply);


    if(result.first){
      nlohmann::json j;
      to_json(j,yutaka_instance);
      std::string stateString = j.dump();

      weilsdk::WeilValue wv;
      wv.new_with_state_and_ok_value(stateString, "Ok");

      weilsdk::Runtime::setStateAndResult(std::variant<weilsdk::WeilValue,std::string> {wv});
    }
    else{
      weilsdk::MethodError me = weilsdk::MethodError("init",result.second);
      weilsdk::Runtime::setStateAndResult(weilsdk::WeilError::FunctionReturnedWithError(me));
    }

  }

  void name(){
    std::string stateString = weilsdk::Runtime::state();
    nlohmann::json j = nlohmann::json::parse(stateString);

    from_json(j,yutaka_instance);

    std::string result = yutaka_instance.getName();
    weilsdk::Runtime::setResult(result,0);
  }

  void symbol(){
    std::string stateString = weilsdk::Runtime::state();
    nlohmann::json j = nlohmann::json::parse(stateString);

    from_json(j,yutaka_instance);

    std::string result = yutaka_instance.getSymbol();
    weilsdk::Runtime::setResult(result,0);
  }

  void decimals(){
    uint8_t result = 6;
    weilsdk::Runtime::setResult(std::to_string(result),0);
  }


  void details(){
    std::string stateString = weilsdk::Runtime::state();
    nlohmann::json j = nlohmann::json::parse(stateString);

    from_json(j,yutaka_instance);

    std::tuple<std::string, std::string, uint8_t> result = yutaka_instance.getDetails();
    nlohmann::json j1 = result;

    weilsdk::Runtime::setResult(j1.dump(),0);
  }

  void total_supply(){
    std::string stateString = weilsdk::Runtime::state();
    nlohmann::json j = nlohmann::json::parse(stateString);

    from_json(j,yutaka_instance);

    uint64_t result = yutaka_instance.getTotalSupply();
    weilsdk::Runtime::setResult(std::to_string(result),0);
  }

  void balance_for(){

    std::pair<std::string, std::string> p = weilsdk::Runtime::stateAndArgs();

    std::string raw_args = p.second;
    nlohmann::json j = nlohmann::json::parse(raw_args);

    if (j.is_discarded() || !j.contains("addr"))
    {
        weilsdk::MethodError me = weilsdk::MethodError("balance_for", "invalid_args");
        weilsdk::Runtime::setResult(weilsdk::WeilError::MethodArgumentDeserializationError(me), 1);
        return;
    }

    balanceForArgs args;
    args = j.get<balanceForArgs>();

    std::string stateString = p.first;
    nlohmann::json j1 = nlohmann::json::parse(stateString);

    from_json(j1,yutaka_instance);

    uint64_t result = yutaka_instance.balanceFor(args.addr);
    weilsdk::Runtime::setResult(std::to_string(result),0);
  }


  void transfer(){

    std::pair<std::string, std::string> p = weilsdk::Runtime::stateAndArgs();

    std::string raw_args = p.second;
    nlohmann::json j = nlohmann::json::parse(raw_args);

    if (j.is_discarded() || !j.contains("to_addr") || !j.contains("amount"))
    {
        weilsdk::MethodError me = weilsdk::MethodError("transfer", "invalid_args");
        weilsdk::Runtime::setResult(weilsdk::WeilError::MethodArgumentDeserializationError(me), 1);
        return;
    }

    transferArgs args;
    args = j.get<transferArgs>();

    std::string stateString = p.first;
    nlohmann::json j1 = nlohmann::json::parse(stateString);

    from_json(j1,yutaka_instance);

    bool result = yutaka_instance.transfer(args.to_addr, args.amount).first;
    if(result){
        nlohmann::json j2 = yutaka_instance;
        weilsdk::WeilValue wv;
        wv.new_with_state_and_ok_value(j2.dump(), "null");
        weilsdk::Runtime::setStateAndResult(std::variant<weilsdk::WeilValue,std::string> {wv});
    }
    else{
      weilsdk::MethodError me = weilsdk::MethodError("transfer","could not transfer");
      std::string err = weilsdk::WeilError::FunctionReturnedWithError(me);
      weilsdk::Runtime::setStateAndResult(std::variant<weilsdk::WeilValue,std::string> {err});
    }
  }

  void approve(){

    std::pair<std::string, std::string> p = weilsdk::Runtime::stateAndArgs();
    std::string raw_args = p.second;

    nlohmann::json j = nlohmann::json::parse(raw_args);

    if (j.is_discarded() || !j.contains("spender") || !j.contains("amount"))
    {
        weilsdk::MethodError me = weilsdk::MethodError("approve", "invalid_args");
        weilsdk::Runtime::setResult(weilsdk::WeilError::MethodArgumentDeserializationError(me), 1);
        return;
    }

    approveArgs args;
    args = j.get<approveArgs>();

    std::string stateString = p.first;
    nlohmann::json j1 = nlohmann::json::parse(stateString);

    from_json(j1,yutaka_instance);

    yutaka_instance.approve(args.spender, args.amount);
    nlohmann::json j2 = yutaka_instance;
    weilsdk::WeilValue wv;
    wv.new_with_state_and_ok_value(j2.dump(), "Ok");
    weilsdk::Runtime::setStateAndResult(std::variant<weilsdk::WeilValue,std::string> {wv});
  }

  void transfer_from(){
    
    std::pair<std::string, std::string> p = weilsdk::Runtime::stateAndArgs();
    std::string raw_args = p.second;

    nlohmann::json j = nlohmann::json::parse(raw_args);


    if (j.is_discarded() || !j.contains("from_addr") || !j.contains("to_addr") || !j.contains("amount"))
    {
        weilsdk::MethodError me = weilsdk::MethodError("transfer_from", "invalid_args");
        weilsdk::Runtime::setResult(weilsdk::WeilError::MethodArgumentDeserializationError(me), 1);
        return;
    }

    transferFromArgs args;
    args = j.get<transferFromArgs>();

    std::string stateString = p.first;
    nlohmann::json j1 = nlohmann::json::parse(stateString);

    from_json(j1,yutaka_instance);

    bool result = yutaka_instance.transferFrom(args.from_addr, args.to_addr, args.amount).first;

    if(result){
        nlohmann::json j2 = yutaka_instance;
        weilsdk::WeilValue wv;
        wv.new_with_state_and_ok_value(j2.dump(), "Ok");
        weilsdk::Runtime::setStateAndResult(std::variant<weilsdk::WeilValue,std::string> {wv});
    }
    else{
      weilsdk::MethodError me = weilsdk::MethodError("transfer_from","could not transfer_from");
      std::string err = weilsdk::WeilError::FunctionReturnedWithError(me);
      weilsdk::Runtime::setStateAndResult(std::variant<weilsdk::WeilValue,std::string> {err});
    }
  }

  void allowance(){
    
    std::pair<std::string, std::string> p = weilsdk::Runtime::stateAndArgs();
    std::string raw_args = p.second;

    nlohmann::json j = nlohmann::json::parse(raw_args);

    if (j.is_discarded() || !j.contains("spender") || !j.contains("owner"))
    {
        weilsdk::MethodError me = weilsdk::MethodError("allowance", "invalid_args");
        weilsdk::Runtime::setResult(weilsdk::WeilError::MethodArgumentDeserializationError(me), 1);
        return;
    }
    allowanceForArgs args;
    args = j.get<allowanceForArgs>();

    std::string stateString = p.first;
    nlohmann::json j1 = nlohmann::json::parse(stateString);

    from_json(j1,yutaka_instance);

    uint64_t result = yutaka_instance.allowance(args.owner, args.spender);
    weilsdk::Runtime::setResult(std::to_string(result),0);
  }
}
