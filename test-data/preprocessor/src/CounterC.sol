// Contract without constructor
contract CounterC {
    struct CounterCStruct {
        address a;
        bool b;
    }
    uint256 public number;

    constructor(string memory _name, uint _age, address _wallet) {}
}
