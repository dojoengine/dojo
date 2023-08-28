# Dojo NPM


#### Packages
- [Dojo Core](./core)
- [Dojo React](./react/)

## Dojo Core

Dojo core aims to be a set of low level reusable functions that integrate seamlessly into a Dojo world. Design goals:

- Simple and non-framework orientated, the core should represent the lowest level of the Dojo js stack

## Dojo React

Dojo React aims to expose a set of React hooks using Dojo Core for seamless integration into a Dojo world for any React based apps.

## Contributing 

You will need to sym link the local packages for them to work correctly

```
cd react && yarn link
cd core && yarn link
```
