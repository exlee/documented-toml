# Model

`model.yml` is the source. Everything else in this directory is generated from
it by `tools/model_gen.rb`.

```sh
ruby tools/model_gen.rb --all      # both outputs
ruby tools/model_gen.rb --puml     # PlantUML only
ruby tools/model_gen.rb --mdj      # StarUML only
```

| File | Tool | Use |
|---|---|---|
| `model.yml` | none | the source, hand-edited |
| `domain.puml`, `decor.puml`, `corpus.puml` | PlantUML | review in a diff, render anywhere |
| `toml-merge.mdj` | StarUML | File > Open, no import step |

## Scopes

**domain** the public API: `MergeOptions`, `Merged`, `Report`, `Diagnostic` and
its kinds, the errors, and the engine's helpers (`SpanIndex`, `Span`).

**decor** the comment machinery: `Marker`, `Prefix`, the `PrefixLine` kinds and
`DocBlock` assembly.

**corpus** the text specification format and the harness that reads it.

## Rust in UML

Rust sum types carrying payloads have no UML equivalent. They appear as an
abstract class stereotyped `«rust enum»` with one subclass per variant, so
`DiagnosticKind`, `Error` and `PrefixLine` each expand into a small hierarchy.
Payload-free Rust enums (`Severity`, `TomlType`, `SectionKind`) are UML
enumerations. `Option<T>` is multiplicity `0..1` and `Vec<T>` is `*`.

## Rendering PlantUML

```sh
java -jar plantuml.jar -tpng -o /tmp/render doc/model/*.puml
java -jar plantuml.jar -syntax < doc/model/domain.puml    # parse check only
```

## StarUML

Open `toml-merge.mdj` directly. Classes, enumerations, attributes, operations
and relationships land in the Model Explorer, grouped into one package per
scope, and each package holds a class diagram already populated with its
elements and their relationships.

A view is more than geometry. The metamodel declares compartments as embedded
references, `attributeCompartment ref UMLAttributeCompartmentView
embedded=subViews`, and `Element#load` swaps the constructor's compartment for
the file's only when that reference is present. A view written without them
keeps the constructor's compartments, which carry no `model`, and the box then
draws empty and cannot be selected. So each node view carries its name,
attribute, operation, reception and template compartments, each edge carries its
three labels, and associations carry six more plus two qualifier compartments,
every one of them named by both `subViews` and its own reference field.

The generator lays views out on a grid. Diagram > Layout in StarUML rearranges
them, and that arrangement is lost on the next regeneration.

Element ids are derived from element names, so they are stable across runs and
regenerating produces a readable diff.

## Visual Paradigm

VP has no Rust support in either direction, and its only programmatic door is
XMI, which creates elements on import instead of reconciling them. So a second
import of a changed file leaves duplicates to merge by hand.

The route for VP is Instant Reverse over Java stubs, which updates classes
already in the model when re-run. `tools/model_gen.rb` has no Java emitter yet;
it is worth adding once the design settles, because until then PlantUML is the
cheaper loop.

## One-way

Generation runs one way. Editing `toml-merge.mdj` in StarUML and then
regenerating discards those edits, including any diagram layout. While the
design is still moving, `model.yml` stays authoritative. When it settles and the
model moves into a tool, generation stops and the tool holds it.
