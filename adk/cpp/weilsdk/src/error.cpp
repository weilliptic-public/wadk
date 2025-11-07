/**
 * @file error.cpp
 * @brief Implementation of error handling classes and error serialization functions
 * @details This file provides implementations for various error types used in the Weil SDK,
 *          including MethodError, ContractCallError, and WeilError, along with JSON serialization
 *          utilities and error message generation methods.
 */

#include "weilsdk/error.h"
#include "external/nlohmann.hpp"

namespace weilsdk {

/**
 * @brief Constructs a MethodError with the given method name and error message
 * @param method_name The name of the method where the error occurred
 * @param err_msg The error message describing what went wrong
 */
MethodError::MethodError(std::string method_name, std::string err_msg)
    : method_name(std::move(method_name)), err_msg(std::move(err_msg)) {}

/**
 * @brief Constructs a ContractCallError with contract ID, method name, and error message
 * @param contract_id The ID of the contract where the error occurred
 * @param method_name The name of the method where the error occurred
 * @param err_msg The error message describing what went wrong
 */
ContractCallError::ContractCallError(std::string contract_id,
                                     std::string method_name,
                                     std::string err_msg)
    : contract_id(std::move(contract_id)), method_name(std::move(method_name)),
      err_msg(std::move(err_msg)) {}

/**
 * @brief Constructs a WeilError with the given error message
 * @param message The error message
 */
WeilError::WeilError(const std::string &message)
    : std::runtime_error(message) {}

/**
 * @brief Serializes a MethodError to JSON format
 * @param error The MethodError object to serialize
 * @return A JSON object containing the method name and error message
 */
nlohmann::json to_json(const MethodError &error) {
  return {{"method_name", error.method_name}, {"err_msg", error.err_msg}};
}

/**
 * @brief Serializes a ContractCallError to JSON format
 * @param error The ContractCallError object to serialize
 * @return A JSON object containing the contract ID, method name, and error message
 */
nlohmann::json to_json(const ContractCallError &error) {
  return {{"contract_id", error.contract_id},
          {"method_name", error.method_name},
          {"err_msg", error.err_msg}};
}

/**
 * @brief Creates a JSON error message for method argument deserialization errors
 * @param error The MethodError containing details about the deserialization failure
 * @return A JSON-formatted string representing the error
 */
std::string
WeilError::MethodArgumentDeserializationError(const MethodError &error) {
  nlohmann::json json_error = {
      {"MethodArgumentDeserializationError", to_json(error)}};
  return json_error.dump();
}

/**
 * @brief Creates a JSON error message when a function returns with an error
 * @param error The MethodError containing details about the function error
 * @return A JSON-formatted string representing the error
 */
std::string WeilError::FunctionReturnedWithError(const MethodError &error) {
  nlohmann::json json_error = {{"FunctionReturnedWithError", to_json(error)}};
  return json_error.dump();
}

/**
 * @brief Creates a JSON error message for WASM module execution trap errors
 * @param error The MethodError containing details about the trap
 * @return A JSON-formatted string representing the error
 */
std::string
WeilError::TrapOccuredWhileWasmModuleExecution(const MethodError &error) {
  nlohmann::json json_error = {
      {"TrapOccuredWhileWasmModuleExecution", to_json(error)}};
  return json_error.dump();
}

/**
 * @brief Creates a JSON error message when a key is not found in a collection
 * @param key The key that was not found
 * @return A JSON-formatted string representing the error
 */
std::string WeilError::KeyNotFoundInCollection(const std::string &key) {
  nlohmann::json json_error = {{"KeyNotFoundInCollection", key}};
  return json_error.dump();
}

/**
 * @brief Creates a JSON error message when no value is returned from deleting a collection item
 * @param key The key of the item that was deleted
 * @return A JSON-formatted string representing the error
 */
std::string
WeilError::NoValueReturnedFromDeletingCollectionItem(const std::string &key) {
  nlohmann::json json_error = {
      {"NoValueReturnedFromDeletingCollectionItem", key}};
  return json_error.dump();
}

/**
 * @brief Creates a JSON error message when no entries are found for keys with a given prefix
 * @param prefix The prefix that was searched for
 * @return A JSON-formatted string representing the error
 */
std::string WeilError::EntriesNotFoundInCollectionForKeysWithPrefix(
    const std::string &prefix) {
  nlohmann::json json_error = {
      {"EntriesNotFoundInCollectionForKeysWithPrefix", prefix}};
  return json_error.dump();
}

/**
 * @brief Creates a JSON error message for contract method execution errors
 * @param error The ContractCallError containing details about the execution failure
 * @return A JSON-formatted string representing the error
 */
std::string
WeilError::ContractMethodExecutionError(const ContractCallError &error) {
  nlohmann::json json_error = {
      {"ContractMethodExecutionError", to_json(error)}};
  return json_error.dump();
}

/**
 * @brief Creates a JSON error message for invalid cross-contract call errors
 * @param error The ContractCallError containing details about the invalid call
 * @return A JSON-formatted string representing the error
 */
std::string
WeilError::InvalidCrossContractCallError(const ContractCallError &error) {
  nlohmann::json json_error = {
      {"InvalidCrossContractCallError", to_json(error)}};
  return json_error.dump();
}

/**
 * @brief Creates a JSON error message for cross-contract call result deserialization errors
 * @param error The ContractCallError containing details about the deserialization failure
 * @return A JSON-formatted string representing the error
 */
std::string WeilError::CrossContractCallResultDeserializationError(
    const ContractCallError &error) {
  nlohmann::json json_error = {
      {"CrossContractCallResultDeserializationError", to_json(error)}};
  return json_error.dump();
}

/**
 * @brief Creates a JSON error message for LLM cluster errors
 * @param message The error message describing the LLM cluster failure
 * @return A JSON-formatted string representing the error
 */
std::string WeilError::LLMClusterError(const std::string &message) {
  nlohmann::json json_error = {{"LLMClusterError", message}};
  return json_error.dump();
}

/**
 * @brief Creates a JSON error message for streaming response deserialization errors
 * @param message The error message describing the deserialization failure
 * @return A JSON-formatted string representing the error
 */
std::string
WeilError::StreamingResponseDeserializationError(const std::string &message) {
  nlohmann::json json_error = {
      {"StreamingResponseDeserializationError", message}};
  return json_error.dump();
}

/**
 * @brief Creates a JSON error message for outcall errors
 * @param message The error message describing the outcall failure
 * @return A JSON-formatted string representing the error
 */
std::string WeilError::OutcallError(const std::string &message) {
  nlohmann::json json_error = {{"OutcallError", message}};
  return json_error.dump();
}

/**
 * @brief Creates a JSON error message for invalid data received errors
 * @param message The error message describing the invalid data
 * @return A JSON-formatted string representing the error
 */
std::string WeilError::InvalidDataReceivedError(const std::string &message) {
  nlohmann::json json_error = {{"InvalidDataReceivedError", message}};
  return json_error.dump();
}

/**
 * @brief Creates a JSON error message for invalid WASM module errors
 * @param message The error message describing the invalid WASM module
 * @return A JSON-formatted string representing the error
 */
std::string WeilError::InvalidWasmModuleError(const std::string &message) {
  nlohmann::json json_error = {{"InvalidWasmModuleError", message}};
  return json_error.dump();
}
} // namespace weilsdk
