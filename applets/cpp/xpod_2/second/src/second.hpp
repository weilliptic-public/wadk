#ifndef SECOND_CONTRACT_HPP
#define SECOND_CONTRACT_HPP

#include "weilsdk/runtime.h"
#include <string>
#include <tuple>
#include "weilsdk/collections/map.hpp"


namespace weilcontracts{

    class Second{
        public:
            collections::WeilMap<std::string, std::vector<uint8_t>> _map;
            
            Second(){
                this->_map = collections::WeilMap<std::string, std::vector<uint8_t>>(0);
            }

            std::vector<uint8_t> get_list(std::string id){
                return this->_map.get(id);
            }
            std::vector<uint8_t> set_val(std::string id, uint8_t val){

                if(this->_map.contains(id)){
                    std::vector<uint8_t> vec = this->_map.get(id);
                    vec.push_back(val);
                    this->_map.insert(id,vec);
                }else{
                    std::vector<uint8_t> vec;
                    vec.push_back(val);
                    this->_map.insert(id,vec);
                }

                return this->_map.get(id);
            }
        
    };

    inline void to_json(nlohmann::json &j, const Second &second){
        j = nlohmann::json::object(); // Empty JSON object
    }
    inline void from_json(const nlohmann::json &j, Second &second){
        //no fields to deserialize
    }
}

#endif