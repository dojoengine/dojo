# `dojoup`

Update or revert to a specific Dojo branch with ease.

## Installing

```sh
curl -L https://install.dojoengine.org | bash
```

## Usage

To install the **nightly** version:

```sh
dojoup
```

To install a specific **version** (in this case the `nightly` version):

```sh
dojoup --version nightly
```

To install a specific **branch** (in this case the `release/0.1.0` branch's latest commit):

```sh
dojoup --branch release/0.1.0
```

To install a **fork's main branch** (in this case `tarrencev/dojo`'s main branch):

```sh
dojoup --repo tarrencev/dojo
```

To install a **specific branch in a fork** (in this case the `patch-10` branch's latest commit in `tarrencev/dojo`):

```sh
dojoup --repo tarrencev/dojo --branch patch-10
```

To install from a **specific Pull Request**:

```sh
dojoup --pr 1071
```

To install from a **specific commit**:

```sh
dojoup -C 94bfdb2
```

To install a local directory or repository (e.g. one located at `~/git/dojo`, assuming you're in the home directory)

##### Note: --branch, --repo, and --version flags are ignored during local installations.

```sh
dojoup --path ./git/dojo
```

---

**Tip**: All flags have a single character shorthand equivalent! You can use `-v` instead of `--version`, etc.

---