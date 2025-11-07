#ifndef ERROR_H
#define ERROR_H

#include "external/nlohmann.hpp"
#include <memory>
#include <stdexcept>
#include <string>

namespace weilsdk {

class MethodError {
public:
  MethodError(std::string method_name, std::string err_msg);
  std::string method_name;
  std::string err_msg;
};

class ContractCallError {
public:
  ContractCallError(std::string contract_id, std::string method_name,
                    std::string err_msg);
  std::string contract_id;
  std::string method_name;
  std::string err_msg;
};

class WeilError : public std::runtime_error {
public:
  WeilError(const std::string &message);

  // Serialization methods for each error type
  static std::string
  MethodArgumentDeserializationError(const MethodError &error);
  static std::string FunctionReturnedWithError(const MethodError &error);
  static std::string
  TrapOccuredWhileWasmModuleExecution(const MethodError &error);
  static std::string KeyNotFoundInCollection(const std::string &key);
  static std::string
  NoValueReturnedFromDeletingCollectionItem(const std::string &key);
  static std::string
  EntriesNotFoundInCollectionForKeysWithPrefix(const std::string &prefix);
  static std::string
  ContractMethodExecutionError(const ContractCallError &error);
  static std::string
  InvalidCrossContractCallError(const ContractCallError &error);
  static std::string
  CrossContractCallResultDeserializationError(const ContractCallError &error);
  static std::string LLMClusterError(const std::string &message);
  static std::string
  StreamingResponseDeserializationError(const std::string &message);
  static std::string OutcallError(const std::string &message);
  static std::string InvalidDataReceivedError(const std::string &message);
  static std::string InvalidWasmModuleError(const std::string &message);
};

} // namespace weilsdk

#endif // ERROR_H
