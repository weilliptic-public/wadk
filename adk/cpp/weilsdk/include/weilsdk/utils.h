#include "weilsdk/runtime.h"
#include "weilsdk/error.h"
#include "external/nlohmann.hpp"
#include <variant>

namespace weilsdk {

    template <typename T>
    using Result = std::variant<T, WeilError>;

    template <typename T>
    Result<T> tryIntoResult(const Result<std::string>& result) {
        // Check if the result is an error
        if (std::holds_alternative<WeilError>(result)) {
            return std::get<WeilError>(result); 
        }

        const std::string& val = std::get<std::string>(result);

        T ok_val = nlohmann::json::parse(val).get<T>();
        return ok_val; 
    }
}