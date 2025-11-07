/**
 * @file vector.hpp
 * @brief Implementation of a persistent vector collection
 * @details This file provides the WeilVec template class, which implements a
 *          persistent vector/array that stores data in the blockchain state.
 *          It supports push, pop, get, set operations with automatic JSON
 *          serialization/deserialization, and provides iterator support.
 */

#ifndef VECTOR_HPP
#define VECTOR_HPP

#include "collections.hpp"
#include "weilsdk/memory.h"
#include "external/nlohmann.hpp"
#include <string>
#include <vector>

extern "C" void write_collection(int key, int val);
__attribute__((import_name("write_collection")));
extern "C" int delete_collection(int key);
__attribute__((import_name("delete_collection")));
extern "C" int read_collection(int key);
__attribute__((import_name("read_collection")));

namespace collections {
  /**
   * @brief A persistent vector/array collection
   * @tparam T The type of elements stored in the vector
   * @details This class provides a persistent vector implementation that stores
   *          elements in the blockchain state. Elements are automatically
   *          serialized to JSON for storage and deserialized when retrieved.
   *          The vector supports standard operations like push, pop, get, and set.
   */
  template <typename T>
  class WeilVec : public collections::Collection<int> {
  private:
    uint8_t state_id; ///< The state ID used to identify this vector in storage
    int len;          ///< The current length of the vector

  public:
    /**
     * @brief Constructs a WeilVec with an uninitialized state ID and zero length
     */
    WeilVec(): state_id(-1), len(0) {}
    
    /**
     * @brief Constructs a WeilVec with the specified state ID and zero length
     * @param id The state ID to use for this vector
     */
    WeilVec(uint8_t id) : state_id(id), len(0) {}

    /**
     * @brief Gets the current size of the vector
     * @return The number of elements in the vector
     */
    int size() const { return len; }
    
    /**
     * @brief Resizes the vector to the specified length
     * @param _len The new length for the vector
     */
    void resize(int _len) { len = _len;}

    /**
     * @brief Gets the base state path for this vector
     * @return The base state path as a string representation of the state ID
     */
    std::string base_state_path() const { return std::to_string(state_id); }
    
    /**
     * @brief Constructs the full state tree key for a given index
     * @param index The index to construct the state tree key for
     * @return The full state tree key as a string
     */
    std::string state_tree_key(const size_t &index) const {
      return base_state_path() + "_" + std::to_string(index);
    }

    /**
     * @brief Appends an element to the end of the vector
     * @param item The element to append
     */
    void push(const T &item) {
      nlohmann::json jsonPayload = item;
      std::string serializedPayload = jsonPayload.dump();

      weilsdk::Memory::writeCollection(state_tree_key(len), serializedPayload);
      len++;
    }

    /**
     * @brief Gets the element at the specified index
     * @param index The index of the element to retrieve
     * @return The element at the specified index, or a default-constructed value if not found
     */
    T get(int index) const {
      auto result = weilsdk::Memory::readCollection(state_tree_key(index));
      std::string s = result.second;
      if (result.first) {
        T t;
        return t;
      }
      nlohmann::json j = nlohmann::json::parse(s);
      T t1 = j.get<T>();
      return t1;
    }
    
    /**
     * @brief Sets the element at the specified index
     * @param index The index of the element to set
     * @param item The new value for the element
     * @details If the index is out of bounds, the operation is silently ignored
     */
    void set(size_t index, const T &item) {
      if (index >= len) {
        return;
      }
      // serialize
      nlohmann::json jsonPayload = item;
      std::string serializedPayload = jsonPayload.dump();

      weilsdk::Memory::writeCollection(state_tree_key(index), serializedPayload);
    }
    
    /**
     * @brief Removes and returns the last element of the vector
     * @return The last element, or a default-constructed value if the vector is empty
     */
    T pop() {

      std::pair<int, std::string> res =
          weilsdk::Memory::deleteCollection(state_tree_key(len-1));
      std::string s = res.second;
      if (res.first) {
        T t1;
        return t1;
      }
      len--;
      nlohmann::json j = nlohmann::json::parse(s);
      return j.get<T>();
    }

    /**
     * @brief Gets the state ID of this vector
     * @return The state ID
     */
    uint8_t getStateId() const{
        return this->state_id;
    }

    /**
     * @brief Sets the state ID of this vector
     * @param _stateId The new state ID to set
     */
    void setStateId(uint8_t _stateId){
      this->state_id = _stateId;
    }

    /**
     * @brief Forward iterator for WeilVec
     * @details This iterator allows forward iteration over the elements of the vector.
     *          It provides standard iterator operations including dereference, increment,
     *          and comparison.
     */
    class iterator {
    private:
      const WeilVec *vector; ///< Pointer to the vector being iterated
      int current;           ///< Current index position

    public:
      using iterator_category = std::forward_iterator_tag; ///< Iterator category
      using difference_type = std::ptrdiff_t;              ///< Difference type
      using value_type = T;                                 ///< Value type
      using pointer = T *;                                  ///< Pointer type
      using reference = T &;                                ///< Reference type

      /**
       * @brief Constructs an iterator for the given vector at the specified index
       * @param v Pointer to the vector to iterate over
       * @param idx The starting index
       */
      iterator(const WeilVec *v, int idx)
          : vector(v), current(idx) {}

      /**
       * @brief Dereferences the iterator to get the current element
       * @return The element at the current index
       */
      T operator*() const {
        return vector->get(current);
      }

      /**
       * @brief Prefix increment operator
       * @return Reference to the incremented iterator
       */
      iterator &operator++() {
        ++current;
        return *this;
      }

      /**
       * @brief Postfix increment operator
       * @return A copy of the iterator before incrementing
       */
      iterator operator++(int) {
        iterator temp = *this;
        ++(*this);
        return temp;
      }

      /**
       * @brief Equality comparison operator
       * @param other The iterator to compare with
       * @return true if both iterators point to the same position, false otherwise
       */
      bool operator==(const iterator &other) const {
        return current == other.current;
      }

      /**
       * @brief Inequality comparison operator
       * @param other The iterator to compare with
       * @return true if the iterators point to different positions, false otherwise
       */
      bool operator!=(const iterator &other) const {
        return current != other.current;
      }
    };

    /**
     * @brief Gets an iterator pointing to the first element
     * @return Iterator pointing to the beginning of the vector
     */
    iterator begin() const { return iterator(this, 0); }

    /**
     * @brief Gets an iterator pointing past the last element
     * @return Iterator pointing to the end of the vector
     */
    iterator end() const { return iterator(this, len); }

    /**
     * @brief Serializes the vector metadata to JSON
     * @param j The JSON object to populate
     */
    inline void to_json(nlohmann::json& j) {
      j = nlohmann::json::object();
      j["state_id"] = this->getStateId();
      j["len"] = this->size();
    }

    /**
     * @brief Deserializes the vector metadata from JSON
     * @param j The JSON object to deserialize from
     */
    inline void from_json(const nlohmann::json& j) {
      this->setStateId(j["state_id"]);
      this->resize(j["len"]);
    }
  };
} 
#endif // VECTOR_HPP
