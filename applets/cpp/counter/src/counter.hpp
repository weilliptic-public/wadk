#include "weilsdk/runtime.h"
#include "external/nlohmann.hpp"
#include "weilsdk/error.h"
#include <stdexcept>

class Counter {
public:
    int value;

    Counter(int initialValue) : value(initialValue) {}
    Counter() : value(0) {}

    int getCount() const {
        return value;
    }

    void increment() {
        value += 1;
    }

    void decrement() {
        if (value > 0) {
            value -= 1;
        }
    }

    void setValue(int new_value) {
        value = new_value;
    }
};

// Serialization functions for Counter
inline void to_json(nlohmann::json& j, const Counter& c) {
    j = nlohmann::json{{"value", c.value}};
}
inline void from_json(const nlohmann::json& j, Counter& c) {
    int val = j.at("value");
    c.value = val;
}

struct setValueArgs{
    int val;
};

inline void to_json(nlohmann::json& j, const setValueArgs& s) {
    j = nlohmann::json{{"val", s.val}};
}
inline void from_json(const nlohmann::json& j, setValueArgs& s) {
    int _val = j.at("val");
    s.val = _val;
}