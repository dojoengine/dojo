# `dojoup`

```sh
curl -L https://install.dojoengine.org | bash
```

For more details, you can then issue the following command:

```sh
dojoup --help
```

[Documentation](https://book.dojoengine.org/getting-started#getting-started)

## Working with Dojoup

To test dojoup, there are two options:

1. Use the `dojoup/Dockerfile` to build a Docker image and run the post-install check.
2. Use the workflow `dojoup.yml` to run the post-install check on a runner.

Currently, dojoup are bash programs being downloaded and executed. Probably in a near future, only the install script will be downloaded and the dojoup logic will be inside a rust binary.
