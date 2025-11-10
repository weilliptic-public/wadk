/**
 * @file runtime.cpp
 * @brief Implementation of runtime operations for WASM contract execution
 * @details This file provides implementations for runtime operations including contract state management,
 *          cross-contract calls, memory allocation/deallocation, and access to blockchain context
 *          information such as sender, block height, and timestamps.
 */

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

    /**
     * @brief Reads bytes from memory at the given pointer with error handling
     * @details This function has core logic to read underlying data. It is separately
     *          defined in both runtime.cpp and memory.cpp instead of being in a header file
     *          to safeguard its visibility from the outside world. This version includes
     *          special handling for WASM module error codes (-1, -2, -3).
     * @param ptr The memory pointer to read from, or an error code (-1, -2, -3)
     * @return A pair where the first element indicates if an error occurred (1) or not (0),
     *         and the second element contains the serialized string data or error message
     */
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

    /**
     * @brief Converts a string to a length-prefixed byte buffer
     * @details This function creates a buffer with an error flag byte, followed by
     *          a 4-byte length, followed by the payload data. This is separately
     *          defined in both runtime.cpp and memory.cpp to safeguard its visibility.
     * @param payload The string data to convert
     * @param is_error Error flag byte (0 for no error, 1 for error)
     * @return A vector of bytes containing the error flag, length, and payload
     */
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

    /**
     * @brief Serializes a string to JSON
     * @param j The JSON object to populate
     * @param s The string to serialize
     */
    void to_json(nlohmann::json &j, const std::string &s) { j = s; }
    
    /**
     * @brief Deserializes JSON to a string
     * @param j The JSON object to deserialize from
     * @param s The string to populate
     */
    void from_json(const nlohmann::json &j, std::string &s) { j.get_to(s); }

    /**
     * @brief Arguments structure for cross-contract calls
     */
    struct CrossContractCallArgs {
      std::string id;          ///< The contract ID to call
      std::string method_name; ///< The method name to invoke
      std::string method_args; ///< Serialized method arguments
    };

    /**
     * @brief Serializes CrossContractCallArgs to JSON
     * @param j The JSON object to populate
     * @param c The cross-contract call arguments to serialize
     */
    void to_json(nlohmann::json &j, const CrossContractCallArgs &c) {
      j = nlohmann::json{{"id", c.id},
                        {"method_name", c.method_name},
                        {"method_args", c.method_args}};
    }
    
    /**
     * @brief Deserializes JSON to CrossContractCallArgs
     * @param j The JSON object to deserialize from
     * @param c The cross-contract call arguments to populate
     */
    void from_json(const nlohmann::json &j, CrossContractCallArgs &c) {
      j.at("id").get_to(c.id);
      j.at("method_name").get_to(c.method_name);
      j.at("method_args").get_to(c.method_args);
    }

    /**
     * @brief Serializes a vector of bytes to JSON
     * @param j The JSON object to populate
     * @param v The vector of bytes to serialize
     */
    void to_json(nlohmann::json &j, const std::vector<uint8_t> &v) {
      j = nlohmann::json::array();
      for (auto &byte : v) {
        j.push_back(byte);
      }
    }
    
    /**
     * @brief Deserializes JSON to a vector of bytes
     * @param j The JSON object to deserialize from
     * @param v The vector of bytes to populate
     */
    void from_json(const nlohmann::json &j, std::vector<uint8_t> &v) {
      v.clear();
      for (const auto &byte : j) {
        v.push_back(byte.get<uint8_t>());
      }
    }

    /**
     * @brief RAII wrapper for managing allocated memory segments
     * @details This class manages the lifetime of dynamically allocated memory,
     *          automatically freeing it when the object goes out of scope.
     */
    class MemorySegment {
      private:
        uint8_t *data_; ///< Raw pointer to allocated memory
        size_t size_;   ///< Size of the allocated memory

      public:
        /**
         * @brief Allocates a memory segment of the specified size
         * @param len The size in bytes to allocate
         * @throws std::bad_alloc if allocation fails
         */
        explicit MemorySegment(size_t len) : size_(len) {
          data_ = static_cast<uint8_t *>(std::malloc(len));
          if (!data_) {
            //should never reach here....
            Runtime::debugLog("bad allocate");
            throw std::bad_alloc();
          }
        }

        /**
         * @brief Frees the allocated memory
         */
        ~MemorySegment() { std::free(data_); }

        /**
         * @brief Gets the raw pointer to the allocated memory
         * @return Pointer to the allocated memory
         */
        uint8_t *get() const { return data_; }

        /**
         * @brief Gets the size of the allocated memory
         * @return The size in bytes
         */
        size_t size() const { return size_; }
    };

    /**
     * @brief Allocates a block of memory of the specified size
     * @param len The size in bytes to allocate
     * @return Pointer to the allocated memory
     */
    uint8_t *Runtime::allocate(size_t len) {
      auto segment = new MemorySegment(len);
      return segment->get();
    }

    /**
     * @brief Frees a block of memory
     * @param ptr Pointer to the memory to free
     */
    static void free_memory(uint8_t *ptr) { std::free(ptr); }

    /**
     * @brief Gets the current contract's ID
     * @return The contract ID as a string
     */
    std::string Runtime::contractId() {
      int32_t ptr = ::get_contract_id();
      std::pair<int, std::string> res = readBytesFromMemory2(ptr);
      return res.second;
    }

    /**
     * @brief Gets the current contract's state
     * @return The contract state as a serialized string
     */
    std::string Runtime::state() {

      int32_t ptr = ::get_state_and_args();
      std::pair<int,std::string> res  = readBytesFromMemory2(ptr);

      nlohmann::json j1 = nlohmann::json::parse(res.second);
      StateArgsValue sav;
      from_json(j1,sav);
      return sav.state;
    }

    /**
     * @brief Gets the arguments passed to the current contract method
     * @return The method arguments as a serialized string
     */
    std::string Runtime::args() {

      int32_t ptr = ::get_state_and_args();
      std::pair<int,std::string> res = readBytesFromMemory2(ptr);

      nlohmann::json j1 = nlohmann::json::parse(res.second);
      StateArgsValue sav;
      from_json(j1,sav);
      return sav.args;
    }

    /**
     * @brief Gets both the contract state and method arguments
     * @return A pair containing the state (first) and args (second) as serialized strings
     */
    std::pair<std::string,std::string> Runtime::stateAndArgs(){
      int32_t ptr = ::get_state_and_args();
      std::pair<int,std::string> res = readBytesFromMemory2(ptr);

      nlohmann::json j1 = nlohmann::json::parse(res.second);
      StateArgsValue sav;
      from_json(j1,sav);
      return {sav.state,sav.args};
    }

    /**
     * @brief Gets the address of the sender of the current transaction
     * @return The sender address as a string
     */
    std::string Runtime::sender() {
      int32_t ptr = ::get_sender();
      std::pair<int, std::string> res = readBytesFromMemory2(ptr);
      return res.second;
    }

    /**
     * @brief Gets the ledger contract ID
     * @return The ledger contract ID as a string
     */
    std::string Runtime::ledgerContractId() {
      int32_t ptr = ::get_ledger_contract_id();
      std::pair<int, std::string> res = readBytesFromMemory2(ptr);
      return res.second;
    }

    /**
     * @brief Gets the current block height
     * @return The block height as an unsigned 64-bit integer
     */
    uint64_t Runtime::blockHeight() {
      int32_t ptr = ::get_block_height();
      std::pair<int, std::string> res = readBytesFromMemory2(ptr);
      return std::stoull(res.second);
    }

    /**
     * @brief Gets the current block timestamp
     * @return The block timestamp as a string
     */
    std::string Runtime::blockTimestamp() {
      int32_t ptr = ::get_block_timestamp();
      std::pair<int, std::string> res = readBytesFromMemory2(ptr);
      return res.second;
    }

    /**
     * @brief Sets the contract state
     * @param state The new state to set, as a serialized string
     */
    void Runtime::setState(std::string state) {
      auto state_bytes = getLengthPrefixedBytesFromString2(state, 0);
      uintptr_t ptr1 = reinterpret_cast<uintptr_t>(state_bytes.data());
      ::set_state(ptr1);
    }

    /**
     * @brief Sets the result of the contract method execution
     * @param result The result string to set
     * @param error Error flag: 1 if result is an error, 0 if result is success
     */
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

    /**
     * @brief Sets both the contract state and the result of method execution
     * @param result The result variant containing either a WeilValue (success) or string (error)
     */
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

    /**
     * @brief Makes a cross-contract call to another contract
     * @param contract_id The ID of the contract to call
     * @param method_name The name of the method to invoke
     * @param method_args The serialized arguments for the method
     * @return A pair where the first element indicates if an error occurred (1) or not (0),
     *         and the second element contains the result or error message
     */
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

    /**
     * @brief Makes a cross-contract call to an XPOD contract
     * @param contract_id The ID of the XPOD contract to call
     * @param method_name The name of the method to invoke
     * @param method_args The serialized arguments for the method
     * @return A pair where the first element indicates if an error occurred (1) or not (0),
     *         and the second element contains the result or error message
     */
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

    /**
     * @brief Deallocates a block of memory
     * @param ptr The pointer to the memory to deallocate
     * @param len The size of the memory block (unused, kept for interface compatibility)
     */
    void Runtime::deallocate(size_t ptr, size_t len) {
        uint8_t* raw_ptr = reinterpret_cast<uint8_t*>(ptr);
        auto deleter = [](uint8_t* p) {
            std::free(p);
        };
        // Transfer ownership to unique_ptr
        std::unique_ptr<uint8_t, decltype(deleter)> buffer(raw_ptr, deleter);
        //the unique_ptr goes out of scope here and is hence dropped.
    }

    /**
     * @brief Logs a debug message
     * @param log The debug message to log
     */
    void Runtime::debugLog(const std::string log) {

      auto raw_log = getLengthPrefixedBytesFromString2(log, 0);
      uintptr_t ptr1 = reinterpret_cast<uintptr_t>(raw_log.data());
      ::debug_log(ptr1);
    }

} // namespace weilsdk
