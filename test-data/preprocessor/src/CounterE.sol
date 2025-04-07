// Contracts with payable constructor
contract CounterE {
    constructor() payable {}
}

contract CounterF {
    constructor(uint256 x) payable {}
}

contract CounterG {
    constructor(address) payable {}
}

