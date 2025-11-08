#include "weilsdk/runtime.h"
#include "weilsdk/error.h"
#include "weilsdk/memory.h"
#include <memory>
#include <cstddef>

// imported from wasm
extern "C" void write_collection(int key, int val);
__attribute__((import_name("write_collection")));
extern "C" int delete_collection(int key);
__attribute__((import_name("delete_collection")));
extern "C" int read_collection(int key);
__attribute__((import_name("read_collection")));
extern "C" int read_bulk_collection(int prefix);
__attribute__((import_name("read_bulk_collection")));
extern "C" int get_state();
__attribute__((import_name("get_state")));
extern "C" int get_args();
__attribute__((import_name("get_args")));
extern "C" int get_state_and_args();
__attribute__((import_name("get_state_and_args")));
extern "C" int get_sender();
__attribute__((import_name("get_sender")));
extern "C" int get_block_height();
__attribute__((import_name("get_block_height")));
extern "C" int get_block_timestamp();
__attribute__((import_name("get_block_timestamp")));
extern "C" int get_contract_id();
__attribute__((import_name("get_contract_id")));
extern "C" int get_ledger_contract_id();
__attribute__((import_name("get_ledger_contract_id")));
extern "C" void set_state(int ptr);
__attribute__((import_name("set_state")));
extern "C" void set_result(int ptr);
__attribute__((import_name("set_result")));
extern "C" void set_state_and_result(int ptr);
__attribute__((import_name("set_state_and_result")));
extern "C" int call_contract(int ptr);
__attribute__((import_name("call_contract")));
extern "C" int call_xpod_contract(int ptr);
__attribute__((import_name("call_xpod_contract")));
extern "C" void debug_log(int log);
__attribute__((import_name("debug_log")));

namespace weilsdk {

    // Helper functions:

    // The below two functions have core logic to read underlying data
    // These two are separately defined in both runtime.cpp and memory.cpp
    // instead to having centrally in a header file
    // This is done specifically to safeguard their visibility to outside world

    std::pair<int, std::string> readBytesFromMemory2(int32_t ptr) {
      switch (ptr) {
        case -1: 
          return {1, weilsdk::WeilError::InvalidWasmModuleError("WASM size limit reached")};
          break;
        case -2: 
          return {1, weilsdk::WeilError::InvalidWasmModuleError("invalid __new function export in module")};
          break;
        case -3: 
          return {1, weilsdk::WeilError::InvalidWasmModuleError("invalid __free function export in module")};
          break;
      }
      uint8_t *mem_ptr = reinterpret_cast<uint8_t *>(ptr);
      uint32_t len = 0;

      std::memcpy(&len, mem_ptr + 1, 4);
      bool is_error = mem_ptr[0];
      std::string serialized_str(reinterpret_cast<char *>(mem_ptr + 1 + 4), len);

      return {is_error, serialized_str};
    }

    std::vector<uint8_t>
    getLengthPrefixedBytesFromString2(const std::string &payload,
                                      uint8_t is_error) {
      std::vector<uint8_t> buffer;
      uint32_t len = payload.size();
      buffer.push_back(is_error);
      buffer.insert(buffer.end(), reinterpret_cast<const uint8_t *>(&len),
                    reinterpret_cast<const uint8_t *>(&len) + 4);
      buffer.insert(buffer.end(), payload.begin(), payload.end());
      return buffer;
    }

    void to_json(nlohmann::json &j, const std::string &s) { j = s; }
    void from_json(const nlohmann::json &j, std::string &s) { j.get_to(s); }

    // For cross contract calls
    struct CrossContractCallArgs {
      std::string id;
      std::string method_name;
      std::string method_args;
    };

    void to_json(nlohmann::json &j, const CrossContractCallArgs &c) {
      j = nlohmann::json{{"id", c.id},
                        {"method_name", c.method_name},
                        {"method_args", c.method_args}};
    }
    void from_json(const nlohmann::json &j, CrossContractCallArgs &c) {
      j.at("id").get_to(c.id);
      j.at("method_name").get_to(c.method_name);
      j.at("method_args").get_to(c.method_args);
    }

    void to_json(nlohmann::json &j, const std::vector<uint8_t> &v) {
      j = nlohmann::json::array();
      for (auto &byte : v) {
        j.push_back(byte);
      }
    }
    void from_json(const nlohmann::json &j, std::vector<uint8_t> &v) {
      v.clear();
      for (const auto &byte : j) {
        v.push_back(byte.get<uint8_t>());
      }
    }

    // MemorySegment class
    class MemorySegment {
      private:
        uint8_t *data_; // Raw pointer to allocated memory
        size_t size_;   // Size of the allocated memory

      public:
        explicit MemorySegment(size_t len) : size_(len) {
          data_ = static_cast<uint8_t *>(std::malloc(len));
          if (!data_) {
            //should never reach here....
            Runtime::debugLog("bad allocate");
            throw std::bad_alloc();
          }
        }

        ~MemorySegment() { std::free(data_); }

        uint8_t *get() const { return data_; }

        size_t size() const { return size_; }
    };

    // Runtime
    uint8_t *Runtime::allocate(size_t len) {
      auto segment = new MemorySegment(len);
      return segment->get();
    }

    static void free_memory(uint8_t *ptr) { std::free(ptr); }

    std::string Runtime::contractId() {
      int32_t ptr = ::get_contract_id();
      std::pair<int, std::string> res = readBytesFromMemory2(ptr);
      return res.second;
    }

    std::string Runtime::state() {

      int32_t ptr = ::get_state_and_args();
      std::pair<int,std::string> res  = readBytesFromMemory2(ptr);

      nlohmann::json j1 = nlohmann::json::parse(res.second);
      StateArgsValue sav;
      from_json(j1,sav);
      return sav.state;
    }

    std::string Runtime::args() {

      int32_t ptr = ::get_state_and_args();
      std::pair<int,std::string> res = readBytesFromMemory2(ptr);

      nlohmann::json j1 = nlohmann::json::parse(res.second);
      StateArgsValue sav;
      from_json(j1,sav);
      return sav.args;
    }

    std::pair<std::string,std::string> Runtime::stateAndArgs(){
      int32_t ptr = ::get_state_and_args();
      std::pair<int,std::string> res = readBytesFromMemory2(ptr);

      nlohmann::json j1 = nlohmann::json::parse(res.second);
      StateArgsValue sav;
      from_json(j1,sav);
      return {sav.state,sav.args};
    }

    std::string Runtime::sender() {
      int32_t ptr = ::get_sender();
      std::pair<int, std::string> res = readBytesFromMemory2(ptr);
      return res.second;
    }

    std::string Runtime::ledgerContractId() {
      int32_t ptr = ::get_ledger_contract_id();
      std::pair<int, std::string> res = readBytesFromMemory2(ptr);
      return res.second;
    }

    uint64_t Runtime::blockHeight() {
      int32_t ptr = ::get_block_height();
      std::pair<int, std::string> res = readBytesFromMemory2(ptr);
      return std::stoull(res.second);
    }

    std::string Runtime::blockTimestamp() {
      int32_t ptr = ::get_block_timestamp();
      std::pair<int, std::string> res = readBytesFromMemory2(ptr);
      return res.second;
    }

    void Runtime::setState(std::string state) {
      auto state_bytes = getLengthPrefixedBytesFromString2(state, 0);
      uintptr_t ptr1 = reinterpret_cast<uintptr_t>(state_bytes.data());
      ::set_state(ptr1);
    }

    void Runtime::setResult(std::string result, int error) {

      if(error){
          weilsdk::Runtime::setStateAndResult(std::variant<weilsdk::WeilValue,std::string> {result});
      }
      else{
          WeilValue wv;
          wv.new_with_ok_value(result);
          Runtime::setStateAndResult(std::variant<weilsdk::WeilValue,std::string> {wv});
      }

    }

    void Runtime::setStateAndResult(std::variant<weilsdk::WeilValue,std::string> result){

      std::string result_string;
      int error;
      if (std::holds_alternative<WeilValue>(result)) {
          error = 0;
          StateResultValue srv = std::get<WeilValue>(result).raw();
          nlohmann::json j1;
          to_json(j1,srv);
          result_string = j1.dump();
      } else {
          error= 1;
          result_string =std::get<std::string>(result);
      }
      auto raw_result = getLengthPrefixedBytesFromString2(result_string, error);
      uintptr_t ptr1 = reinterpret_cast<uintptr_t>(raw_result.data());
      ::set_state_and_result(ptr1);
    }

    std::pair<int, std::string>
    Runtime::callContract(const std::string contract_id,
                          const std::string method_name,
                          const std::string method_args) {
      CrossContractCallArgs args{contract_id, method_name, method_args};

      nlohmann::json jsonPayload;
      to_json(jsonPayload, args);
      std::string serializedPayload = jsonPayload.dump();
      auto args_buf = getLengthPrefixedBytesFromString2(serializedPayload, 0);
      uintptr_t ptr1 = reinterpret_cast<uintptr_t>(args_buf.data());

      auto result_ptr = ::call_contract(ptr1);
      std::pair<int, std::string> res = readBytesFromMemory2(result_ptr);
      int is_error = res.first;
      std::string serialized_result = res.second;

      if (is_error) {
        ContractCallError cce(contract_id, method_name, serialized_result);
        return {1, weilsdk::WeilError::CrossContractCallResultDeserializationError(
                      cce)};
      } else {
        return {0, serialized_result};
      }
    }

    std::pair<int, std::string>
    Runtime::callXpodContract(const std::string contract_id,
                          const std::string method_name,
                          const std::string method_args) {
      CrossContractCallArgs args{contract_id, method_name, method_args};

      nlohmann::json jsonPayload;
      to_json(jsonPayload, args);
      std::string serializedPayload = jsonPayload.dump();
      auto args_buf = getLengthPrefixedBytesFromString2(serializedPayload, 0);
      uintptr_t ptr1 = reinterpret_cast<uintptr_t>(args_buf.data());

      auto result_ptr = ::call_xpod_contract(ptr1);
      std::pair<int, std::string> res = readBytesFromMemory2(result_ptr);
      int is_error = res.first;
      std::string serialized_result = res.second;

      if (is_error) {
        ContractCallError cce(contract_id, method_name, serialized_result);
        return {1, weilsdk::WeilError::CrossContractCallResultDeserializationError(
                      cce)};
      } else {
        return {0, serialized_result};
      }
    }

    void Runtime::deallocate(size_t ptr, size_t len) {
        uint8_t* raw_ptr = reinterpret_cast<uint8_t*>(ptr);
        auto deleter = [](uint8_t* p) {
            std::free(p);
        };
        // Transfer ownership to unique_ptr
        std::unique_ptr<uint8_t, decltype(deleter)> buffer(raw_ptr, deleter);
        //the unique_ptr goes out of scope here and is hence dropped.
    }

    void Runtime::debugLog(const std::string log) {

      auto raw_log = getLengthPrefixedBytesFromString2(log, 0);
      uintptr_t ptr1 = reinterpret_cast<uintptr_t>(raw_log.data());
      ::debug_log(ptr1);
    }

} // namespace weilsdk
