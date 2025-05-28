# *Public Ledger for Auctions*

Este projeto foi desenvolvido no âmbito da Unidade Curricular **Segurança de Sistemas e Dados (SSD)** do 2º semestre do 1º ano do **Mestrado em Segurança Informática (MSI)** da **Faculdade de Ciências da Universidade do Porto (FCUP)**, no ano letivo 2024/2025.

## Informação do Grupo

**Grupo:** 3

**Membros:**
- Eduardo Luís Fernandes Roçadas (up202108758)
- Leonardo Araújo Freitas (up202400832)
- Manuel Ramos Leite Carvalho Neto (up202108744)

## Instruções de Instalação e Execução

**Requisitos:**
- [Rust](https://www.rust-lang.org/)
- [protoc](https://protobuf.dev/)

```
make clean
make
```

### Nó *Bootstrap*

```
make bootstrap <PORTO>
```

Por exemplo:

```
make bootstrap 5000
```

### Nó

```
make run <PORTO> <PORTO BOOTSTRAP>
```

Por exemplo:

```
make peer 5001 5000
```

### Injeção de Falhas

```
make shutdown <PORTO_1> <...> <PORTO_N>
```

Por exemplo:

```
make shutdown 5001 5002
```

## Estrutura

```
.
|___build.rs
|___Cargo.lock
|___Cargo.toml
|___Makefile
|___README.md
|___proto
    |___kademlia.proto
|___src
    |___constants.rs
    |___lib.rs
    |___auctions
        |___mod.rs
        |___auction.rs
        |___auction_commands.rs
    |___bin
        |___bootstrap.rs
        |___peer.rs
        |___shutdown.rs
    |___kademlia
        |___mod.rs
        |___kbucket.rs
        |___node.rs
        |___routing_table.rs
        |___service.rs
    |___ledger
        |___mod.rs
        |___block.rs
        |___blockchain.rs
        |___hashable.rs
        |___lib.rs
        |___merkle_tree.rs
        |___transaction.rs
        |___transaction_pool.rs
```
