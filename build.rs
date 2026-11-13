fn main() {
    tonic_prost_build::configure()
        .build_server(false)
        .bytes(".preconf.SubscribePreconfsRequest.accounts_of_interest")
        .bytes(".preconf.SubscribePreconfsRequest.programs_of_interest")
        .bytes(".preconf.SubscribePreconfsRequest.excluded_accounts")
        .bytes(".preconf.SubscribePreconfsRequest.signers")
        .bytes(".preconf.SubscribePreconfsRequest.excluded_signers")
        .bytes(".preconf.ValidatorSlotStart.leader_address")
        // Keeps the transaction payload out of a memcpy on its way to the caller.
        .bytes(".preconf.Preconf.data")
        .bytes(".preconf.Preconf.leader")
        .compile_protos(&["protos/preconf.proto"], &["protos"])
        .expect("compile preconfirmation protocol");
}
