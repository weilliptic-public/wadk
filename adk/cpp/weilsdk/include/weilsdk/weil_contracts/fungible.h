#ifndef WEILCONTRACTS_FUNGIBLETOKEN_HPP
#define WEILCONTRACTS_FUNGIBLETOKEN_HPP

#include "external/nlohmann.hpp"
#include "weilsdk/collections/map.hpp"
#include "weilsdk/error.h"
#include "weilsdk/ledger.h"
#include "weilsdk/runtime.h"
#include <string>

namespace weilcontracts {

    struct TokenDetails {
        std::string name;
        std::string symbol;
        uint8_t decimal;
    };

    inline void to_json(nlohmann::json &j, const TokenDetails &td) {
        j = nlohmann::json{
            {"name", td.name},
            {"symbol", td.symbol},
            {"decimal", td.decimal}
        };
    }

    inline void from_json(const nlohmann::json &j, TokenDetails &td) {
        j.at("name").get_to(td.name);
        j.at("symbol").get_to(td.symbol);
        j.at("decimal").get_to(td.decimal);
    }

    // template <typename K,typename V>
    inline void to_json(nlohmann::json& j, const collections::WeilMap<std::string, uint64_t>& m) {
        j = nlohmann::json::object();
        j["state_id"] = m.getStateId();
    }
    // template <typename K,typename V>
    inline void from_json(const nlohmann::json& j, collections::WeilMap<std::string, uint64_t>& m) {
        m.setStateId(j["state_id"]);
    }

    // inline void to_json(nlohmann::json &j, const collections::WeilMap<std::string, uint64_t> &m) {
    //     j = nlohmann::json::object();
    // }

    // inline void from_json(const nlohmann::json &j, collections::WeilMap<std::string, uint64_t> &m) {
    //     for (auto it = j.begin(); it != j.end(); ++it) {
    //         std::string key = it.key();
    //         uint64_t value = it.value();
    //         m.insert(key, value);
    //     }
    // }

    class FungibleToken {
    private:
        std::string name;
        std::string symbol;
        uint64_t totalSupply;
        collections::WeilMap<std::string, uint64_t> allowances;

    public:
        FungibleToken() = default;
        FungibleToken(std::string name, std::string symbol);

        std::string getName() const;
        std::string getSymbol() const;
        uint8_t getDecimals() const;
        TokenDetails getDetails() const;
        uint64_t getTotalSupply() const;
        void setName(std::string _name);
        void setSymbol(std::string _symbol);
        void setTotalSupply(uint64_t _supply);

        collections::WeilMap<std::string,uint64_t> getAllowances() const {return allowances;};
        void setAllowances(collections::WeilMap<std::string, uint64_t> _allowances);
        
        uint64_t balanceFor(std::string addr);
        std::pair<bool, std::string> transfer(std::string toAddr, uint64_t amount);
        void approve(std::string spender, uint64_t amount);
        std::pair<bool, std::string> mint(uint64_t amount);

        std::pair<bool, std::string> transferFrom(std::string fromAddr, std::string toAddr, uint64_t amount);
        uint64_t getAllowance(std::string owner, std::string spender);
    };

    inline void to_json(nlohmann::json &j, const weilcontracts::FungibleToken &tkn) {
        nlohmann::json j1;
        collections::WeilMap<std::string,uint64_t> mp = tkn.getAllowances();
        // to_json<std::string, uint64_t>(j1,mp);
        to_json(j1,mp);
        std::string allowances = j1.dump();

        j = nlohmann::json{
            {"name", tkn.getName()},
            {"symbol", tkn.getSymbol()},
            {"totalSupply", tkn.getTotalSupply()},
            {"allowances", allowances}
        };
    }

    inline void from_json(const nlohmann::json &j, weilcontracts::FungibleToken &tkn) {
        std::string name = j.at("name").get<std::string>();
        std::string symbol = j.at("symbol").get<std::string>();
        uint64_t totalSupply = j.at("totalSupply").get<uint64_t>();


        std::string str = j.at("allowances");
        nlohmann::json j1 = nlohmann::json::parse(str);
        collections::WeilMap<std::string, uint64_t> allowances;
        from_json(j1,allowances);

        tkn = FungibleToken(name, symbol);
        tkn.setAllowances(allowances);
    }
} 

#endif // WEILCONTRACTS_FUNGIBLETOKEN_HPP
