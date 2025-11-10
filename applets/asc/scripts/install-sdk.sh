mkdir ../contract-sdk
cd ../contract-sdk || exit
git init
git remote add -f origin git@github.com:weilliptic-inc/contract-sdk.git
git config core.sparseCheckout true
git sparse-checkout set asc/weil-sdk
git checkout dev
git pull --depth=1
cd -
cd ../contract-sdk/asc/weil-sdk/
npm i
cd -