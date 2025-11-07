#include "weilsdk/runtime.h"
#include "external/nlohmann.hpp"
#include "weilsdk/error.h"

class BContractState{
    public:
        
        std::string generate_greetings_1(std::string name){
            return "From 1: Hello"+name;
        }
        std::string generate_greetings_2(std::string name){
            return "From 2: Hello"+name;
        }
        std::string generate_greetings_3(std::string name){
            weilsdk::Runtime::debugLog("entered B");
            return "From 3: Hello"+name;
        }
};

struct GreetingsArgs{
    std::string name;
};

inline void to_json(nlohmann::json &j, const GreetingsArgs &ga){
    j = nlohmann::json {
        {"name", ga.name}
    };
}

inline void from_json(const nlohmann::json &j, GreetingsArgs &ga) {
    std::string name = j.at("name").get<std::string>();
    ga.name = name;
}