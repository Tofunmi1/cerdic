// Session-local call patterns. Returned tips are consumed once, even if delivery is deferred.
export function createHints() {
  const sent = new Set();
  let previous, guides, singles, applies, failures, nodes, bundledGuide;
  function reset() {
    sent.clear();
    previous = null;
    guides = singles = applies = failures = nodes = 0;
    bundledGuide = false;
  }
  reset();
  function note(tool, args = {}, isError = false) {
    const afterView = previous === 'set_view';
    previous = tool;
    applies = tool === 'apply_text' ? applies + 1 : 0;
    failures = tool === 'apply_text' && isError ? failures + 1 : 0;
    if (tool === 'get_nodes') nodes++;
    if (tool === 'get_guide') {
      guides++;
      // The initial core/menu call is normal orientation; repeated explicit topics can be combined.
      if (typeof args?.topic === 'string' && args?.topics === undefined) singles++;
      if (args?.operators !== undefined || (Array.isArray(args?.topics) && args.topics.length > 1)) bundledGuide = true;
    }
    const candidates = [
      ['syntax', failures >= 3, 'Tip: apply_text({text: "...", dry_run: true}) checks syntax first; get_guide({topic: "compact"}) explains the language.'],
      ['guide', !bundledGuide && ((tool === 'get_guide' && typeof args?.topic === 'string' && singles >= 2) || (tool === 'list_operators' && guides > 0)),
        'Tip: get_guide takes topics: ["curves","terrain"] and operators: {family: "solid"} in one call.'],
      ['cameras', tool === 'snapshot' && afterView && !args?.cameras,
        'Tip: snapshot({cameras: [{position: [3,3,3], target: [0,0,0]}]}) sets the camera and captures it in one call.'],
      ['batch', applies >= 3, 'Tip: batch({steps: [{tool: "apply_text", args: {text: "..."}}]}) combines edits; run({code: "..."}) supports loops and conditions.'],
      ['nodes', tool === 'get_nodes' && nodes >= 3, 'Tip: find({query: "name"}) locates nodes; get_text({}) reads the document in one call.'],
    ];
    for (const [kind, ready, tip] of candidates) {
      if (ready && !sent.has(kind)) { sent.add(kind); return tip; }
    }
    return null;
  }
  return { note, reset };
}
