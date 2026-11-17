# Native sample banks

The native app resolves the sample-bank name stored in an RBS song against
`.rbm` files in `Mods` beside the song/application, nearby song folders, or
the folder selected by `RB338_MOD_DIR`.

`Trancer.rbm` comes from
[Peff's ReBirth archive](https://github.com/peffre/ReBirth-RB-338/blob/17b579c4c2669e17742eab6884712b08b6684fba/ReBirth%20Mods/Trancer.rbm).
Its sample data is unchanged. Its storage key has been rewritten using the
OpenRB-338 recryption tool so this bank can load without `RB20FUL.DAT`.
The original mod's information and credits are retained inside the bank.
The archive's repository license is available
[upstream](https://github.com/peffre/ReBirth-RB-338/blob/17b579c4c2669e17742eab6884712b08b6684fba/LICENSE);
the GPL-3.0 text is retained in
[the native license folder](../licenses/OpenRB-338-GPL-3.0.txt).

Original, unrecrypted banks may require their supported `RB20FUL.DAT` beside
the bank, or a path supplied by `RB338_RB20FUL_DAT`. A missing or invalid bank
is reported before replacing the current song.
