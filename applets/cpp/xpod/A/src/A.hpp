#include "weilsdk/runtime.h"
#include "external/nlohmann.hpp"
#include "weilsdk/error.h"


class AContractState{
    public:
        std::string prefix;

    public:
        AContractState(){
            this->prefix = "A";
        }

        std::pair<bool, std::string> greetings(std::string name, std::string contract_addr){

            struct Args{
                std::string name;
            };

            Args args{name};
            nlohmann::json j;
            j["name"] = args.name;
            std::string serialized_args = j.dump();

            return weilsdk::Runtime::callContract(contract_addr,
                                   "generate_greetings_3",
                                   serialized_args
            );
        }

        std::pair<bool, std::string> x_greetings(std::string name, std::string contract_addr){
            struct Args{
                std::string name;
            };

            Args args{name};
            nlohmann::json j;
            j["name"] = args.name;
            std::string serialized_args = j.dump();

            std::pair<bool, std::string> result =  weilsdk::Runtime::callXpodContract(
                contract_addr,
                "generate_greetings_3",
                serialized_args
            );
            return result;
        }

        void x_greetings_callback(std::string result){
            weilsdk::Runtime::debugLog("xpod greetings result is "+result);
        }
};

inline void to_json(nlohmann::json &j, const AContractState &acs){
    j = nlohmann::json {
        {"prefix", acs.prefix}
    };
}

inline void from_json(const nlohmann::json &j, AContractState &acs) {
    std::string prefix = j.at("prefix").get<std::string>();
    acs.prefix = prefix;
}

struct GreetingsArgs{
    std::string name;
    std::string contract_addr;
};


inline void to_json(nlohmann::json &j, const GreetingsArgs &ga){
    j = nlohmann::json {
        {"name", ga.name},
        {"contract_addr", ga.contract_addr}
    };
}

inline void from_json(const nlohmann::json &j, GreetingsArgs &ga) {
    std::string name = j.at("name").get<std::string>();
    std::string contract_addr = j.at("contract_addr").get<std::string>();

    ga.name = name;
    ga.contract_addr = contract_addr;
}

