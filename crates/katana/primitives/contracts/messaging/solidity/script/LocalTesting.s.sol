// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.0;

import "forge-std/Script.sol";

import "src/Contract1.sol";
import "src/StarknetMessagingLocal.sol";


/**
   Deploys the Contract1 and StarknetMessagingLocal contracts.
*/
contract LocalSetup is Script {
    function setUp() public {}

    function run() public{
        uint256 deployerPrivateKey = vm.envUint("ACCOUNT_PRIVATE_KEY");

        string memory json = "local_testing";

        vm.startBroadcast(deployerPrivateKey);

        address snLocalAddress = address(new StarknetMessagingLocal());
        vm.serializeString(json, "sncore_address", vm.toString(snLocalAddress));

        address contract1 = address(new Contract1(snLocalAddress));
        vm.serializeString(json, "contract1_address", vm.toString(contract1));

        vm.stopBroadcast();

        string memory data = vm.serializeBool(json, "success", true);

        string memory localLogs = "./logs/";
        vm.createDir(localLogs, true);
        vm.writeJson(data, string.concat(localLogs, "local_setup.json"));
    }
}
