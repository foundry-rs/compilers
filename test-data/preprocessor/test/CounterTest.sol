import {Counter} from "src/Counter.sol";
import {Counter as CounterV1} from "src/v1/Counter.sol";
import "src/CounterB.sol";
import "src/CounterC.sol";
import "src/CounterD.sol";

contract CounterTest {
    Counter public counter;
    Counter public counter2 = new Counter();
    CounterB public counter3 = new CounterB(address(this), 44, true, address(this));
    CounterB public counter4 = new CounterB({a:address(this), b: 44, c: true, d:   address(this)});
    CounterV1 public counterv1;
    Counter public counter5 = new Counter{salt: bytes32("123")}();
    CounterB public counter6 = new CounterB {salt: bytes32("123")}   (address(this), 44, true, address(this));

    function setUp() public {
        counter = new Counter();
        counterv1 = new CounterV1(     );
        type(CounterV1).creationCode;
        CounterB counterB = new CounterB(address(this), 15,           false, address(counter));
        CounterC counterC = new CounterC(
            "something",
            35,
            address(this)
        );
        CounterD counterD = new CounterD(address(this), 15, 15);
    }
}