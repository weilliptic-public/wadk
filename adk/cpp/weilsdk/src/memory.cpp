/**
 * @file memory.cpp
 * @brief Implementation of memory and collection operations for WASM runtime
 * @details This file provides implementations for reading and writing to collections,
 *          along with helper functions for memory management and data serialization.
 *          It interfaces with WASM-imported functions for low-level memory operations.
 */

#include "weilsdk/memory.h"

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
extern "C" int call_contract(int ptr);
__attribute__((import_name("call_contract")));
extern "C" void debug_log(int log);
__attribute__((import_name("debug_log")));

namespace weilsdk {

/**
 * @brief Reads bytes from memory at the given pointer
 * @details This function has core logic to read underlying data. It is separately
 *          defined in both runtime.cpp and memory.cpp instead of being in a header file
 *          to safeguard its visibility from the outside world.
 * @param ptr The memory pointer to read from
 * @return A pair where the first element indicates if an error occurred (1) or not (0),
 *         and the second element contains the serialized string data
 */
std::pair<int, std::string> readBytesFromMemory(int32_t ptr) {
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
getLengthPrefixedBytesFromString(const std::string &payload, uint8_t is_error) {
  std::vector<uint8_t> buffer;
  uint32_t len = payload.size();
  buffer.push_back(is_error);
  buffer.insert(buffer.end(), reinterpret_cast<const uint8_t *>(&len),
                reinterpret_cast<const uint8_t *>(&len) + 4);
  buffer.insert(buffer.end(), payload.begin(), payload.end());
  return buffer;
}

/**
 * @brief Reads all collection entries that have keys with the given prefix
 * @param prefix The prefix to search for in collection keys
 * @return A pair where the first element indicates if an error occurred (1) or not (0),
 *         and the second element contains the serialized result data
 */
std::pair<int, std::string> Memory::readBulkCollection(std::string prefix) {
  auto raw_prefix = getLengthPrefixedBytesFromString(prefix, 0);
  uintptr_t ptr1 = reinterpret_cast<uintptr_t>(raw_prefix.data());

  int32_t ptr = ::read_bulk_collection(ptr1);

  auto buffer = readBytesFromMemory(ptr);
  return buffer;
}

/**
 * @brief Writes a key-value pair to the collection
 * @param key The key to write to
 * @param val The value to write
 */
void Memory::writeCollection(std::string key, std::string val) {

  auto raw_key = getLengthPrefixedBytesFromString(key, 0);
  auto raw_val = getLengthPrefixedBytesFromString(val, 0);

  uintptr_t key_ptr = reinterpret_cast<uintptr_t>(raw_key.data());
  uintptr_t val_ptr = reinterpret_cast<uintptr_t>(raw_val.data());

  ::write_collection(key_ptr, val_ptr);
}

/**
 * @brief Deletes a key-value pair from the collection
 * @param key The key to delete
 * @return A pair where the first element indicates if an error occurred (1) or not (0),
 *         and the second element contains the deleted value or error message
 */
std::pair<int, std::string> Memory::deleteCollection(std::string key) {
  auto raw_key = getLengthPrefixedBytesFromString(key, 0);

  // raw_key.data() gets the pointer to byte vector
  uintptr_t ptr = reinterpret_cast<uintptr_t>(raw_key.data());
  int result_ptr = ::delete_collection(ptr);

  auto buffer = readBytesFromMemory(result_ptr);
  return buffer;
}

/**
 * @brief Reads a value from the collection using the given key
 * @param key The key to read from
 * @return A pair where the first element indicates if an error occurred (1) or not (0),
 *         and the second element contains the value or error message
 */
std::pair<int, std::string> Memory::readCollection(std::string key) {
  auto raw_key = getLengthPrefixedBytesFromString(key, 0);

  uintptr_t ptr1 = reinterpret_cast<uintptr_t>(raw_key.data());

  int32_t ptr = ::read_collection(ptr1);
  auto buffer = readBytesFromMemory(ptr);
  return buffer;
}

// Unimplemented

// template <typename T>
// std::optional<T> Memory::read_prefix_for_trie(const std::string& prefix) {
//     try {
//         std::string buffer = Memory::readBulkCollection<T>(prefix);
//         return nlohmann::json::parse(buffer).get<T>();
//     } catch (const std::runtime_error& e) {
//         if
//         (std::string(e.what()).find("EntriesNotFoundInCollectionForKeysWithPrefix")
//         != std::string::npos) {
//             return std::nullopt;
//         }
//         throw e;
//     }
// }
} // namespace weilsdk