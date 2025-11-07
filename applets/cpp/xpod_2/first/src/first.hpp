#ifndef FIRST_CONTRACT_HPP
#define FIRST_CONTRACT_HPP

#include "weilsdk/runtime.h"
#include <string>
#include "weilsdk/collections/map.hpp"


namespace weilcontracts{

    class First{
        public:
            collections::WeilMap<std::string, std::string> xpod_mapping; // xpod_id -> id
            collections::WeilMap<std::string, uint32_t> total_mapping; // id -> counter

            First(){
                this->xpod_mapping.setStateId(0);
                this->total_mapping.setStateId(1);
            }

            std::string health_check(){
                return "Success!";
            }

            //intented to return {is_error, value}
            std::pair<bool, uint32_t> counter(std::string id){
                
                if(total_mapping.contains(id)){
                    return {0,total_mapping.get(id)};
                }
                // 0 for default
                else return {1,0};
            }

            void set_list_in_second(std::string contract_id, std::string id, int val){
                nlohmann::json j;
                j["id"] = id;
                j["val"]= val;
                std::string serialized_args = j.dump();

                std::pair<bool, std::string> result = weilsdk::Runtime::callXpodContract(
                    contract_id,
                    "set_val",
                    serialized_args
                );

                std::string xpod_id = result.second;
                weilsdk::Runtime::debugLog("xpod id is {}" + xpod_id);

                if(!total_mapping.contains(id)){
                    total_mapping.insert(id, 0);
                }
                xpod_mapping.insert(xpod_id, id);
            }

            void set_list_in_second_callback(std::string xpod_id, std::variant<std::string, std::vector<uint8_t>> result){
                // weilsdk::Runtime::debugLog("xpod set list result is "+result);
                if(std::holds_alternative<std::vector<uint8_t>>(result)){
                    if(!xpod_mapping.contains(xpod_id)) return;

                    std::string id = xpod_mapping.get(xpod_id);

                    if(!total_mapping.contains(id)){
                        weilsdk::Runtime::debugLog("unreachable!");
                        return;
                    }
                    int counter = total_mapping.get(id);
                    total_mapping.insert(id,counter+1);
                }
            }
        
    };

    inline void to_json(nlohmann::json &j, const First &first){
        j = nlohmann::json::object(); // Empty JSON object
    }
    inline void from_json(const nlohmann::json &j, First &first){
        //no fields to deserialize
    }
}

#endif