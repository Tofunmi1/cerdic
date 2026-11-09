// inventory: deterministic item stacks stored in the game's serializable script state.
// Declare it with game.script@1(lib="inventory") before scripts that call inventory.*.
//
//   inventory.define("herb", { max_stack: 8, weight: 0.2 })
//   inventory.create(hero, { slots: 12, max_weight: 20 })
//   inventory.add(hero, "herb", 2)       -> { added: 2, left: 0 }
//   inventory.remove(hero, "herb", 1)    -> 1
//   inventory.count(hero, "herb")        -> 1
//   inventory.has(hero, "herb", 1)       -> true
//   inventory.list(hero)                  -> [{ item: "herb", count: 1, weight: 0.2 }]
// Slots count distinct item stacks; max_weight is optional. Failed capacity is returned as `left`.
(() => {
  'use strict';

  const data = () => (state.inventory ??= {
    definitions: Object.create(null),
    containers: Object.create(null),
  });
  const options = (value, label) => {
    if (value === null || typeof value !== 'object' || Array.isArray(value)) throw new Error(label + ' must be an object');
    return value;
  };
  const itemId = (value) => {
    if (typeof value !== 'string' || !/^[a-z][a-z0-9_]{0,47}$/.test(value)) {
      throw new Error('item id must start with a lowercase letter and contain only lowercase letters, digits or underscores');
    }
    return value;
  };
  const entityId = (value) => {
    const id = typeof value === 'number' ? value : value && value.id;
    if (!Number.isSafeInteger(id) || id < 0) throw new Error('inventory operation needs an entity or nonnegative entity id');
    return String(id);
  };
  const positiveInteger = (value, label) => {
    if (!Number.isSafeInteger(value) || value < 1 || value > 1000000) throw new Error(label + ' must be a whole number from 1 to 1000000');
    return value;
  };
  const count = (owner, item) => {
    item = itemId(item);
    definition(item);
    return container(owner).items[item] ?? 0;
  };
  const definition = (id) => {
    const found = data().definitions[itemId(id)];
    if (!found) throw new Error("unknown item '" + id + "': define it with inventory.define first");
    return found;
  };
  const container = (owner) => {
    const found = data().containers[entityId(owner)];
    if (!found) throw new Error('entity has no inventory: call inventory.create first');
    return found;
  };
  const currentWeight = (bag) => Object.keys(bag.items).reduce((total, id) => total + definition(id).weight * bag.items[id], 0);

  globalThis.inventory = {
    define(id, spec = {}) {
      id = itemId(id);
      spec = options(spec, 'item properties');
      const maxStack = spec.max_stack === undefined ? 99 : positiveInteger(spec.max_stack, 'max_stack');
      const weight = spec.weight === undefined ? 0 : spec.weight;
      if (!Number.isFinite(weight) || weight < 0) throw new Error('weight must be a finite nonnegative number');
      const next = { max_stack: maxStack, weight };
      const previous = data().definitions[id];
      if (previous && (previous.max_stack !== next.max_stack || previous.weight !== next.weight)) {
        throw new Error("item '" + id + "' was already defined with different properties");
      }
      data().definitions[id] = next;
      return id;
    },

    create(owner, spec = {}) {
      const id = entityId(owner);
      spec = options(spec, 'inventory limits');
      const slots = spec.slots === undefined ? 20 : positiveInteger(spec.slots, 'slots');
      const maxWeight = spec.max_weight === undefined ? null : spec.max_weight;
      if (maxWeight !== null && (!Number.isFinite(maxWeight) || maxWeight < 0)) {
        throw new Error('max_weight must be a finite nonnegative number');
      }
      const previous = data().containers[id];
      if (previous) {
        if (previous.slots !== slots || previous.max_weight !== maxWeight) throw new Error('inventory already exists with different limits');
        return id;
      }
      if (slots > 1024) throw new Error('slots must not exceed 1024');
      data().containers[id] = { slots, max_weight: maxWeight, items: Object.create(null) };
      return id;
    },

    add(owner, item, amount = 1) {
      item = itemId(item);
      const def = definition(item);
      amount = positiveInteger(amount, 'amount');
      const bag = container(owner);
      const existing = bag.items[item] ?? 0;
      if (existing === 0 && Object.keys(bag.items).length >= bag.slots) return { added: 0, left: amount };
      const byStack = Math.max(0, def.max_stack - existing);
      const byWeight = bag.max_weight === null || def.weight === 0
        ? amount
        : Math.max(0, Math.floor((bag.max_weight - currentWeight(bag) + 1e-9) / def.weight));
      const added = Math.min(amount, byStack, byWeight);
      if (added > 0) bag.items[item] = existing + added;
      return { added, left: amount - added };
    },

    remove(owner, item, amount = 1) {
      item = itemId(item);
      definition(item);
      amount = positiveInteger(amount, 'amount');
      const bag = container(owner);
      const removed = Math.min(amount, bag.items[item] ?? 0);
      const remaining = (bag.items[item] ?? 0) - removed;
      if (remaining > 0) bag.items[item] = remaining;
      else delete bag.items[item];
      return removed;
    },

    count(owner, item) {
      return count(owner, item);
    },

    has(owner, item, amount = 1) {
      return count(owner, item) >= positiveInteger(amount, 'amount');
    },

    weight(owner) {
      return currentWeight(container(owner));
    },

    list(owner) {
      const bag = container(owner);
      return Object.keys(bag.items).sort().map((item) => ({ item, count: bag.items[item], weight: definition(item).weight * bag.items[item] }));
    },
  };
})();
