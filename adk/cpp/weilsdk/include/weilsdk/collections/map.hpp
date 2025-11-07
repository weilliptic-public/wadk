/**
 * @file map.hpp
 * @brief Implementation of a persistent key-value map collection
 * @details This file provides the WeilMap template class, which implements a
 *          persistent key-value map that stores data in the blockchain state.
 *          It supports insert, get, remove, and contains operations with automatic
 *          JSON serialization/deserialization.
 */

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
  /**
   * @brief A persistent key-value map collection
   * @tparam K The type of keys in the map
   * @tparam V The type of values in the map
   * @details This class provides a persistent map implementation that stores
   *          key-value pairs in the blockchain state. Values are automatically
   *          serialized to JSON for storage and deserialized when retrieved.
   */
  template <typename K, typename V>
  class WeilMap : public collections::Collection<K> {
  private:
    uint8_t state_id; ///< The state ID used to identify this map in storage

  public:
    /**
     * @brief Constructs a WeilMap with an uninitialized state ID
     */
    WeilMap() : state_id(-1) {}
    
    /**
     * @brief Constructs a WeilMap with the specified state ID
     * @param id The state ID to use for this map
     */
    WeilMap(uint8_t id) : state_id(id) {}

    /**
     * @brief Gets the base state path for this map
     * @return The base state path as a string representation of the state ID
     */
    std::string base_state_path() const override { 
      std::string res =  std::to_string(state_id); 
      return res;
    }
    
    /**
     * @brief Constructs the full state tree key for a given key
     * @details If the key type is std::string, it's used directly. Otherwise,
     *          the key is serialized to JSON and used as part of the state key.
     * @param key The key to construct the state tree key for
     * @return The full state tree key as a string
     */
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

    /**
     * @brief Inserts or updates a key-value pair in the map
     * @param key The key to insert or update
     * @param value The value to associate with the key
     */
    void insert(const K &key, const V &value) {

      nlohmann::json jsonPayload = value;
      std::string serializedPayload = jsonPayload.dump();
      std::string state_key = state_tree_key(key);
      weilsdk::Memory::writeCollection(state_tree_key(key), serializedPayload);
    }

    /**
     * @brief Gets the state ID of this map
     * @return The state ID
     */
    uint8_t getStateId() const{
      return this->state_id;
    }

    /**
     * @brief Sets the state ID of this map
     * @param _stateId The new state ID to set
     */
    void setStateId(uint8_t _stateId){
      this->state_id = _stateId;
    }

    /**
     * @brief Checks if the map contains a specific key
     * @param key The key to check for
     * @return true if the key exists in the map, false otherwise
     */
    bool contains(const K &key) {
      std::string state_key = state_tree_key(key);
      std::pair<int,std::string> result  =  weilsdk::Memory::readCollection(state_key);
      return !result.first;
    }

    /**
     * @brief Gets the value associated with a key
     * @param key The key to look up
     * @return The value associated with the key, or a default-constructed value if not found
     */
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

    /**
     * @brief Removes a key-value pair from the map
     * @param key The key to remove
     * @return The value that was associated with the key, or a default-constructed value if not found
     */
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

    /**
     * @brief Serializes the map metadata to JSON
     * @param j The JSON object to populate
     */
    inline void to_json(nlohmann::json& j) {
      j = nlohmann::json::object();
      j["state_id"] = getStateId();
    }

    /**
     * @brief Deserializes the map metadata from JSON
     * @param j The JSON object to deserialize from
     */
    inline void from_json(const nlohmann::json& j) {
      setStateId(j["state_id"]);
    }
  };
} // namespace collections
#endif // MAP_HPP
