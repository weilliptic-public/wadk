#ifndef RUNTIME_H
#define RUNTIME_H

#include "external/nlohmann.hpp"
#include <memory>
#include <optional>
#include <string>
#include <variant>
#include <vector>
#include "error.h"

namespace weilsdk {

  //we keep the serialized versions of the state, args and value
  //so for custom structs, the user needs to ensure they are serializable/deserializable
  //usinng nlohmann::json
  struct StateArgsValue {
    std::string state;
    std::string args;
  };

  struct StateResultValue{
    std::string state;
    std::string value;
  };

  inline void to_json(nlohmann::json &j, const StateArgsValue &sav) {
    j["state"] = sav.state;
    j["args"] = sav.args;
  }
  inline void from_json(const nlohmann::json &j, StateArgsValue &sav) {
    sav.state = j["state"];
    sav.args = j["args"];
  }
  inline void to_json(nlohmann::json &j, const StateResultValue &srv) {
      // If state is exactly the string "null", set to JSON null
      if (srv.state == "null" || srv.state == "") {
          j["state"] = nullptr;
      }
      // Otherwise, serialize the state normally
      else {
          j["state"] = srv.state;
      }
      j["value"] = srv.value;
  }

  inline void from_json(const nlohmann::json &j, StateResultValue &srv) {
      // If state is null or not present, set to "null"
      if (!j.contains("state") || j["state"].is_null()) {
          srv.state = "null";
      } 
      // Otherwise, convert to string
      else {
          srv.state = j["state"].get<std::string>();
      }
      srv.value = j["value"];
  }

  //weilvalue
  class WeilValue{

    public:
    std::string state;
    std::string ok_val;

    WeilValue(): state("null"), ok_val(""){};
    WeilValue(std::string _state, std::string _ok_val){
        this->state = _state;
        this->ok_val = _ok_val;
    }

    void new_with_ok_value(std::string _val){
      this->state = "null";
      this->ok_val = _val;
    }

    void new_with_state_and_ok_value(std::string _state, std::string _val){
      this->state = _state;
      this->ok_val = _val;
    }

    bool has_state(){
      return state!="null";
    }

    StateResultValue raw(){
      StateResultValue srv{
        state,
        ok_val,
      };
      return srv;
    }
  };


  inline void to_json(nlohmann::json &j, const WeilValue &wv) {
    if(wv.state=="null"){
      j["state"]=nullptr;
    }
    else j["state"] = wv.state;
    j["ok_val"] = wv.ok_val;
  }
  inline void from_json(const nlohmann::json &j, WeilValue &wv) {
    wv.ok_val = j["ok_val"];
    if (j.contains("state") && !j["state"].is_null()) {
        wv.state = j["state"];
    } else {
        wv.state = "null";
    }
  }

  // Runtime class
  class Runtime {
    public:
      static uint8_t *allocate(size_t len);
      static void deallocate(size_t ptr, size_t len);

      static std::string contractId();
      static std::string state();
      static std::string args();
      static std::pair<std::string,std::string> stateAndArgs();
      static std::string sender();
      static std::string ledgerContractId();
      static uint64_t blockHeight();
      static std::string blockTimestamp();

      static void setState(std::string state);
      static void setResult(std::string result, int error);

      static void setStateAndResult(std::variant<weilsdk::WeilValue, std::string> result);

      static std::pair<int, std::string> callContract(const std::string contractId,
                                                      const std::string methodName,
                                                      const std::string methodArgs);

      static std::pair<int, std::string> callXpodContract(const std::string contractId,
                                                          const std::string methodName,
                                                          const std::string methodArgs);

      static void debugLog(std::string log);
  };

} // namespace weilsdk

#endif
