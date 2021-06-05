# Account store

Our account store needs to store enough information for WebAuth.
At the very minimum that means `Username` -> {`CredentialId` -> `Credential`}.
Need to read the spec more.

We definitely want minimal dependencies and bureaucracy and maximal troubleshooting ability.
We might just go with a JSON file per account, for the data that only changes when users add/remove authenticators.

Need to worry about enterprise use cases, federated authentication, but that's after the basics work.

Write the details here...
