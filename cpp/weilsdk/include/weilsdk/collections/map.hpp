#ifndef MAP_HPP
#define MAP_HPP

#include "collections.hpp"
#include "external/nlohmann.hpp"
#include "weilsdk/memory.h"
#include "weilsdk/runtime.h"
#include <map>
#include <string>

extern "C" void write_collection(int key, int val);
__attribute__((import_name("write_collection")));
extern "C" int delete_collection(int key);
__attribute__((import_name("delete_collection")));
extern "C" int read_collection(int key);
__attribute__((import_name("read_collection")));

namespace collections {
  template <typename K, typename V>
  class WeilMap : public collections::Collection<K> {
  private:
    uint8_t state_id;

  public:
    WeilMap() : state_id(-1) {}
    WeilMap(uint8_t id) : state_id(id) {}

    std::string base_state_path() const override { 
      std::string res =  std::to_string(state_id); 
      return res;
    }
    
    std::string state_tree_key(const K &key) const {
      if constexpr (std::is_same<K, std::string>::value) {
          // If the key is already a string, return it without extra quotes
          std::string res = base_state_path() + "_" + key;
          return res;
      } else {
          // Otherwise, convert key to JSON string and append
          std::string res = base_state_path() + "_" + nlohmann::json(key).dump();
          return res;
      }
    }

    void insert(const K &key, const V &value) {

      nlohmann::json jsonPayload = value;
      std::string serializedPayload = jsonPayload.dump();
      std::string state_key = state_tree_key(key);
      weilsdk::Memory::writeCollection(state_tree_key(key), serializedPayload);
    }

    uint8_t getStateId() const{
      return this->state_id;
    }

    void setStateId(uint8_t _stateId){
      this->state_id = _stateId;
    }

    bool contains(const K &key) {
      std::string state_key = state_tree_key(key);
      std::pair<int,std::string> result  =  weilsdk::Memory::readCollection(state_key);
      return !result.first;
    }

    V get(const K &key) const {
      auto result = weilsdk::Memory::readCollection(state_tree_key(key));
      std::string s = result.second;
      if (weilsdk::Memory::readCollection(state_tree_key(key)).first) {
        V v;
        return v;
      }
      nlohmann::json j = nlohmann::json::parse(s);
      V v1 = j.get<V>();

      return v1;
    }

    V remove(const K &key) {
      std::pair<int, std::string> res =
          weilsdk::Memory::deleteCollection(state_tree_key(key));
      std::string s = res.second;
      if (res.first) {
        V v;
        return v;
      }
      nlohmann::json j = nlohmann::json::parse(s);
      return j.get<V>();
    }

    inline void to_json(nlohmann::json& j) {
      j = nlohmann::json::object();
      j["state_id"] = getStateId();
    }

    inline void from_json(const nlohmann::json& j) {
      setStateId(j["state_id"]);
    }
  };
} // namespace collections
#endif // MAP_HPP
