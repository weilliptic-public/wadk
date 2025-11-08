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
  template <typename T>
  class WeilVec : public collections::Collection<int> {
  private:
    uint8_t state_id;
    int len;

  public:
    WeilVec(): state_id(-1), len(0) {}
    WeilVec(uint8_t id) : state_id(id), len(0) {}

    int size() const { return len; }
    void resize(int _len) { len = _len;}

    std::string base_state_path() const { return std::to_string(state_id); }
    std::string state_tree_key(const size_t &index) const {
      return base_state_path() + "_" + std::to_string(index);
    }

    void push(const T &item) {
      nlohmann::json jsonPayload = item;
      std::string serializedPayload = jsonPayload.dump();

      weilsdk::Memory::writeCollection(state_tree_key(len), serializedPayload);
      len++;
    }

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
    void set(size_t index, const T &item) {
      if (index >= len) {
        return;
      }
      // serialize
      nlohmann::json jsonPayload = item;
      std::string serializedPayload = jsonPayload.dump();

      weilsdk::Memory::writeCollection(state_tree_key(index), serializedPayload);
    }
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

    uint8_t getStateId() const{
        return this->state_id;
    }

    void setStateId(uint8_t _stateId){
      this->state_id = _stateId;
    }

    class iterator {
    private:
      const WeilVec *vector;
      int current;

    public:
      using iterator_category = std::forward_iterator_tag;
      using difference_type = std::ptrdiff_t;
      using value_type = T;
      using pointer = T *;
      using reference = T &;

      iterator(const WeilVec *v, int idx)
          : vector(v), current(idx) {}

      // Dereference operator
      T operator*() const {
        return vector->get(current);
      }

      // Increment operator (prefix)
      iterator &operator++() {
        ++current;
        return *this;
      }

      // Increment operator (postfix)
      iterator operator++(int) {
        WeilVecIterator temp = *this;
        ++(*this);
        return temp;
      }

      // Equality comparison
      bool operator==(const iterator &other) const {
        return current == other.current;
      }

      // Inequality comparison
      bool operator!=(const iterator &other) const {
        return current != other.current;
      }
    };

    iterator begin() const { return WeilVecIterator(this, 0); }

    iterator end() const { return WeilVecIterator(this, len); }

    inline void to_json(nlohmann::json& j) {
      j = nlohmann::json::object();
      j["state_id"] = this->getStateId();
      j["len"] = this->size();
    }

    inline void from_json(const nlohmann::json& j) {
      this->setStateId(j["state_id"]);
      this->resize(j["len"]);
    }
  };
} 
#endif // VECTOR_HPP
