#include "weilsdk/runtime.h"
#include "external/nlohmann.hpp"
#include "weilsdk/error.h"
#include "weilsdk/collections/vector.hpp"

class Euclid {
public:
    collections::WeilVec<int> vec;

    Euclid() {}
    Euclid(uint8_t id) : vec(id) {}

    int size() const {
        return this->vec.size();
    }

    void add(int value) {
        this->vec.push(value);
    }

    int get(int index) const {
        return this->vec.get(index);
    }

    void set(int index, int value) {
        if (index < this->vec.size()) {
            this->vec.set(index, value);
        } else {
            return;
        }
    }

    int remove_last() {
        return this->vec.pop();
    }

    void clear() {
        while (this->vec.size() > 0) {
            this->vec.pop();
        }
    }

    void reset(uint8_t new_id) {
        this->vec.setStateId(new_id);
        clear();
    }

    // Sum of all elements using iterator
    int sum_all() const {
        int sum = 0;
        for (auto it = this->vec.begin(); it!= this->vec.end(); ++it) {
            sum += *it;
        }
        return sum;
    }
};

inline void to_json(nlohmann::json& j, const Euclid& e) {
    j = nlohmann::json{
        {"state_id", e.vec.getStateId()},
        {"size", e.vec.size()},
    };
}

// Deserialization function
inline void from_json(const nlohmann::json& j, Euclid& e) {
    uint8_t state_id = j["state_id"].get<uint8_t>();
    e.vec.setStateId(state_id);

    int size = j["size"].get<int>();
    e.vec.resize(size);
}

struct addArgs{
    int elem;
};

inline void to_json(nlohmann::json &j, const addArgs &k)
{
    j = nlohmann::json{{"elem", k.elem}};
}

inline void from_json(const nlohmann::json &j, addArgs &k)
{
    j.at("elem").get_to(k.elem);
}

struct resetArgs{
    int new_size;
};

inline void to_json(nlohmann::json &j, const resetArgs &k)
{
    j = nlohmann::json{{"new_size", k.new_size}};
}

inline void from_json(const nlohmann::json &j, resetArgs &k)
{
    j.at("new_size").get_to(k.new_size);
}