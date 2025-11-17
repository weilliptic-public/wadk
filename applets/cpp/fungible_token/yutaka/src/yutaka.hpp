#ifndef YUTAKA_CONTRACT_HPP
#define YUTAKA_CONTRACT_HPP

#include "weilsdk/weil_contracts/fungible.h"
#include "weilsdk/runtime.h"
#include <string>
#include <tuple>

namespace weilcontracts {

    class Yutaka {
    private:

    public:
        FungibleToken inner;

        Yutaka(const FungibleToken &inner_) : inner(inner_) {}
        Yutaka();

        std::string getName();
        std::string getSymbol();
        uint8_t getDecimals();
        std::tuple<std::string, std::string, uint8_t> getDetails();
        uint64_t getTotalSupply();
        uint64_t balanceFor(const std::string &addr);

        const FungibleToken& getInner() const { return inner; }
        void setInner(const FungibleToken& token) { inner = token; }
        
        std::pair<bool, std::string> transfer(const std::string &toAddr, uint64_t amount);
        void approve(const std::string &spender, uint64_t amount);
        std::pair<bool, std::string> transferFrom(const std::string &fromAddr, const std::string &toAddr, uint64_t amount);
        uint64_t allowance(const std::string &owner, const std::string &spender);

        friend void from_json(const nlohmann::json& j, Yutaka& yutaka);
        friend void to_json(nlohmann::json &j, const Yutaka &yutaka);

    };

    inline void to_json(nlohmann::json &j, const Yutaka &yutaka) {
        j = nlohmann::json{
            {"inner", yutaka.getInner()}
        };
    }

    inline void from_json(const nlohmann::json &j, Yutaka &yutaka) {
        std::string name = j.at("inner").at("name").get<std::string>();
        std::string symbol = j.at("inner").at("symbol").get<std::string>();
        uint64_t totalSupply = j.at("inner").at("totalSupply").get<uint64_t>();
        std::string allowances = j.at("inner").at("allowances").get<std::string>();

        FungibleToken token(name, symbol);
        yutaka.setInner(token);
    }


} 


namespace weilcontracts {

    Yutaka::Yutaka(){
        FungibleToken token("Yutaka", "YTK");
        this->setInner(token);
   }

    std::string Yutaka::getName() {
        return inner.getName();
    }

    std::string Yutaka::getSymbol() {
        return inner.getSymbol();
    }

    uint8_t Yutaka::getDecimals() {
        return inner.getDecimals();
    }

    std::tuple<std::string, std::string, uint8_t> Yutaka::getDetails() {
        return std::make_tuple(inner.getName(), inner.getSymbol(), 6);
    }

    uint64_t Yutaka::getTotalSupply() {
        return inner.getTotalSupply();
    }

    uint64_t Yutaka::balanceFor(const std::string &addr) {
        return inner.balanceFor(addr);
    }

    std::pair<bool, std::string> Yutaka::transfer(const std::string &toAddr, uint64_t amount) {
        return inner.transfer(toAddr, amount);
    }

    void Yutaka::approve(const std::string &spender, uint64_t amount) {
        inner.approve(spender, amount);
    }

    std::pair<bool, std::string> Yutaka::transferFrom(const std::string &fromAddr, const std::string &toAddr, uint64_t amount) {
        return inner.transferFrom(fromAddr, toAddr, amount);
    }

    uint64_t Yutaka::allowance(const std::string &owner, const std::string &spender){
        return inner.getAllowance(owner, spender);
    }


} 

#endif
