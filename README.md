# Minecraft Redstone JSON-to-lithematic converter

A small project which converts a json Minecraft redstone project to lithematic or nbt.

The project json is used by repository [MinecraftRedstoneEditor](https://github.com/dianfeng-junguan/MinecraftRedstoneEditor.git)

template project json
```json

{
    "name":"template Circuit",
    "size":{"x":10,"y":10,"z":10},
    "imports":[
        {
            "modelName":"and",
            "modelType":"component",
            "path":"./components/and.json"
        },
        ...
    ],

    "components":[
        {
            "name":"and001",
            "model":"and",
            "position":{
                "x":0,
                "y":1,
                "z":2
            }
        }
    ],
    "wires":[
        {
            "name":"wire001",
            "start":{
                "x":1,
                "y":1,
                "z":2
            
            },
            "end":{
                "x":2,
                "y":1,
                "z":2
            },
            "baseMaterial":"stone"
        }
    ],
    "blocks":[
        {
            "position":{"x":0,"y":0,"z":0},
            "id":"repeator",
            "properties":{
                "facing":"south",
                "delay":1,
                "locked":false,
                "powered":false
            }
        }
    ],
    "inputs":[
        {"name":"input001","position":{"x":0,"y":0,"z":0}},
        ...
    ],
    "outputs":[
        {"name":"output001","position":{"x":1,"y":0,"z":0}},
        ...
    ]
}

```
## Structure of a project json

### imports

#### modelName

name of the model of the component.

#### type

type of the component model.
- component：Regular component corresponding to an NBT file.
- circuit: Subcircuit described by a json which depends on other components.

#### path

path of the NBT file of the model. If relative, it will search for the nbt in the lib/.

### circuit

#### components

#### wires

##### baseMaterial

block name such as "stone"。

#### blocks

Some blocks you might want to place apart from components.

### inputs



### outputs


the "input" and "output" are necessary when the circuit project is referred to as subcircuit, optional when it's just a project.

## Format of component JSON
For example, and.json
```json
{
    "name":"and",
    "modelType":"component",
    "nbt":"and.nbt",
    "size":[2,2,2],
    "inputs":[
        {"x":0,"y":0,"z":0},
        ...
    ],
    "outputs":[
        {"x":1,"y":0,"z":0},
        ...
    ]
}
```
the "nbt" here can be nbt, lithematic or schematic.

## Compile

```bash
cargo build
```

## Usage

see it by running
```bash
./mc_circuit_script -h
```


