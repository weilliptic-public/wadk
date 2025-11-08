#include "weilsdk/error.h"
#include "external/nlohmann.hpp"

namespace weilsdk {

MethodError::MethodError(std::string method_name, std::string err_msg)
    : method_name(std::move(method_name)), err_msg(std::move(err_msg)) {}

ContractCallError::ContractCallError(std::string contract_id,
                                     std::string method_name,
                                     std::string err_msg)
    : contract_id(std::move(contract_id)), method_name(std::move(method_name)),
      err_msg(std::move(err_msg)) {}

WeilError::WeilError(const std::string &message)
    : std::runtime_error(message) {}

// Method to serialize MethodError to JSON
nlohmann::json to_json(const MethodError &error) {
  return {{"method_name", error.method_name}, {"err_msg", error.err_msg}};
}

// Method to serialize ContractCallError to JSON
nlohmann::json to_json(const ContractCallError &error) {
  return {{"contract_id", error.contract_id},
          {"method_name", error.method_name},
          {"err_msg", error.err_msg}};
}

std::string
WeilError::MethodArgumentDeserializationError(const MethodError &error) {
  nlohmann::json json_error = {
      {"MethodArgumentDeserializationError", to_json(error)}};
  return json_error.dump();
}

std::string WeilError::FunctionReturnedWithError(const MethodError &error) {
  nlohmann::json json_error = {{"FunctionReturnedWithError", to_json(error)}};
  return json_error.dump();
}

std::string
WeilError::TrapOccuredWhileWasmModuleExecution(const MethodError &error) {
  nlohmann::json json_error = {
      {"TrapOccuredWhileWasmModuleExecution", to_json(error)}};
  return json_error.dump();
}

std::string WeilError::KeyNotFoundInCollection(const std::string &key) {
  nlohmann::json json_error = {{"KeyNotFoundInCollection", key}};
  return json_error.dump();
}

std::string
WeilError::NoValueReturnedFromDeletingCollectionItem(const std::string &key) {
  nlohmann::json json_error = {
      {"NoValueReturnedFromDeletingCollectionItem", key}};
  return json_error.dump();
}

std::string WeilError::EntriesNotFoundInCollectionForKeysWithPrefix(
    const std::string &prefix) {
  nlohmann::json json_error = {
      {"EntriesNotFoundInCollectionForKeysWithPrefix", prefix}};
  return json_error.dump();
}

std::string
WeilError::ContractMethodExecutionError(const ContractCallError &error) {
  nlohmann::json json_error = {
      {"ContractMethodExecutionError", to_json(error)}};
  return json_error.dump();
}

std::string
WeilError::InvalidCrossContractCallError(const ContractCallError &error) {
  nlohmann::json json_error = {
      {"InvalidCrossContractCallError", to_json(error)}};
  return json_error.dump();
}

std::string WeilError::CrossContractCallResultDeserializationError(
    const ContractCallError &error) {
  nlohmann::json json_error = {
      {"CrossContractCallResultDeserializationError", to_json(error)}};
  return json_error.dump();
}

std::string WeilError::LLMClusterError(const std::string &message) {
  nlohmann::json json_error = {{"LLMClusterError", message}};
  return json_error.dump();
}

std::string
WeilError::StreamingResponseDeserializationError(const std::string &message) {
  nlohmann::json json_error = {
      {"StreamingResponseDeserializationError", message}};
  return json_error.dump();
}

std::string WeilError::OutcallError(const std::string &message) {
  nlohmann::json json_error = {{"OutcallError", message}};
  return json_error.dump();
}

std::string WeilError::InvalidDataReceivedError(const std::string &message) {
  nlohmann::json json_error = {{"InvalidDataReceivedError", message}};
  return json_error.dump();
}

std::string WeilError::InvalidWasmModuleError(const std::string &message) {
  nlohmann::json json_error = {{"InvalidWasmModuleError", message}};
  return json_error.dump();
}
} // namespace weilsdk
