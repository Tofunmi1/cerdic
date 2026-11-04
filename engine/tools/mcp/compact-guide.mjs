import { readFileSync } from 'node:fs';

const example = name => '```text\n' + readFileSync(new URL(`./guide-examples/compact_${name}.txt`, import.meta.url), 'utf8').trimEnd() + '\n```';

export const COMPACT = {
  about: 'pipes, loops (numbers and text), join, if/else, def macros, constants and expressions',
  text: `Pipes and loops make a long build short. Operator versions may be omitted to use the latest version.

A pipe feeds each stage into the first input of the next. The final stage takes the statement's name; earlier stages are named post_1, post_2, and so on. Those names must be free. A line ending in | continues on the next line.
${example('pipes')}

A range includes its start and excludes its end. Use let constants and {expression} arithmetic in values; {i} in node and output names must be a whole number.
${example('range')}

A list visits each item. Negative numbers in names become m1 for -1.
${example('list')}

Unpack pairs (or triples) to use several loop variables:
${example('pairs')}

Nested loops see the outer variables:
${example('nested')}

A loop can walk text as well as numbers. {side} in a node name is the text (letters, digits and underscores only); anywhere else it is that text as a string, and \${side} inside a string is spliced:
${example('text')}

join builds a text from a loop, for the lists and strings you would otherwise write by hand. The first argument is a loop header, the second a template ({expression} is a number, \${name} a constant), the optional third the separator (default ", "). \${name} is spliced as written, inside a string or outside one:
${example('join')}

if / else reads only the branch that holds. Compare numbers (< <= > >= == !=) or text (== !=), join with && and ||, negate with !, group with parentheses; a lone number is true when it is not zero. It is decided from constants and loop variables, never from the model; a let inside a branch stays inside it:
${example('if')}

def writes a block once and a call reads it again with its parameters bound to the arguments (numbers, expressions, "text", constants; "n\${i}" builds a text). The body may hold nodes, let, for, if and calls of other macros, and sees the constants above the call; in a body \${name} is raw text (an input: anim.track(\${prev}, ...)) and output {name} = ... takes a name. A macro never calls itself, a def stands at the top level, and a body holds no group, expose, result or def:
${example('macro')}

Limits: at most 2,000 statements after expansion (macro calls count) and loop nesting depth 3; no groups in loops, ifs or macros. Loop bodies may contain node statements, let, output, nested loops, ifs and macro calls. For larger repetition use inst.array or inst.radial. A bone's own joint is pivot_at="head" on anim.track (no typed pivot); value.point reads a bone's head or tail, or a part's bounding box, as a number that links into any number parameter (by.y=hy).

get_text defaults to format="compact": it omits latest-version suffixes and default parameters, prints exact short angles in degrees when possible, and folds eligible chains into pipes. format="canonical" returns the stored form; the viewer's Text panel stays canonical. Both forms read back to the same document.`,
};
