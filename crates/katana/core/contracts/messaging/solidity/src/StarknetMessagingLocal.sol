// SPDX-License-Identifier: Apache-2.0.
pragma solidity ^0.8.0;

import "starknet/StarknetMessaging.sol";

/**
   @notice Interface related to local messaging for Starknet.
*/
interface IStarknetMessagingLocal {
    function addMessageHashFromL2(
        uint256 msgHash
    )
        external
        payable;
}

/**
   @title A superset of StarknetMessaging to support
   local development by adding a way to directly register
   a message hash ready to be consumed, without waiting the block
   to be verified.

   @dev The idea is that, to not wait on the block to be proved,
   this messaging contract can receive directly a hash of a message
   to be considered as `received`. This message can then be consumed.

   DISCLAIMER:
   The purpose of this contract is for local development only.
*/
contract StarknetMessagingLocal is StarknetMessaging, IStarknetMessagingLocal {

    /**
       @notice A message hash was added directly.
    */
    event MessageHashAddedFromL2(
        bytes32 indexed messageHash
    );

    /**
       @notice Adds the hash of a message from L2.

       @param msgHash Hash of the message to be considered as consumable.
    */
    function addMessageHashFromL2(
        uint256 msgHash
    )
        external
        payable
    {
        // TODO: You can add here a whitelist of senders if you wish.
        bytes32 hash = bytes32(msgHash);
        emit MessageHashAddedFromL2(hash);
        l2ToL1Messages()[hash] += 1;
    }

}
