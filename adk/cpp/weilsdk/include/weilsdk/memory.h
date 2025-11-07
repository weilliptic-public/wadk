#ifndef MEMORY_H
#define MEMORY_H

#include "external/nlohmann.hpp"
#include <memory>
#include <optional>
#include <string>
#include <variant>
#include <vector>

namespace weilsdk {

class Memory {
    public:
        static std::pair<int, std::string>
        readBulkCollection(const std::string prefix);
        static void writeCollection(std::string key, std::string val);
        static std::pair<int, std::string> deleteCollection(std::string key);
        static std::pair<int, std::string> readCollection(std::string key);

        // Unimplemented
        /*
        template <typename T>
        static std::optional<T> readPrefixForTrie(std::string_view prefix) {
            try {
                std::string buffer = Memory::readBulkCollection(prefix);
                return nlohmann::json::parse(buffer).get<T>();
            } catch (const std::runtime_error& e) {
                if
        (std::string(e.what()).find("EntriesNotFoundInCollectionForKeysWithPrefix") !=
        std::string::npos) { return std::nullopt;
                }
                throw e;
            }
        }
        */
    };
} // namespace weilsdk
#endif