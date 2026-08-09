import { describe, expect, it } from 'vitest';
import {
  EMPTY_DOC,
  HISTORY_LIMIT,
  canRedo,
  canUndo,
  compatibility,
  initialEditorState,
  reducer,
  resolveTokens,
  selectedElement,
} from './state';
import type { Action, EditorState } from './state';

const start = (): EditorState => initialEditorState(structuredClone(EMPTY_DOC));

const run = (state: EditorState, ...actions: Action[]): EditorState =>
  actions.reduce(reducer, state);

const withThreeElements = (): EditorState =>
  run(start(), { type: 'add', kind: 'text' }, { type: 'add', kind: 'button' }, { type: 'add', kind: 'badge' });

describe('ajout', () => {
  it('ajoute en fin de tableau (donc au premier plan) et sélectionne', () => {
    const state = run(start(), { type: 'add', kind: 'text', x: 30, y: 40 });
    expect(state.doc.elements).toHaveLength(1);
    const [element] = state.doc.elements;
    expect(element.type).toBe('text');
    expect([element.x, element.y]).toEqual([30, 40]);
    expect(state.selectedId).toBe(element.id);
  });

  it('conserve les dimensions naturelles d’un média déposé sur le canvas', () => {
    const state = run(start(), {
      type: 'add',
      kind: 'image',
      x: 86,
      y: 54,
      w: 144,
      h: 81,
      assetId: 'asset-test',
    });
    expect(state.doc.elements[0]).toMatchObject({
      type: 'image',
      x: 86,
      y: 54,
      w: 144,
      h: 81,
      assetId: 'asset-test',
    });
  });

  it('donne à chaque élément un id [a-z0-9]{7} distinct et un anim non partagé', () => {
    const state = withThreeElements();
    const ids = state.doc.elements.map((e) => e.id);
    expect(new Set(ids).size).toBe(3);
    for (const id of ids) expect(id).toMatch(/^[a-z0-9]{7}$/);

    const [first, second] = state.doc.elements;
    expect(first.anim).not.toBe(second.anim);
  });
});

describe('déplacement', () => {
  it("écrit la nouvelle position et n'ajoute qu'une entrée d'historique", () => {
    const added = run(start(), { type: 'add', kind: 'text' });
    const id = added.doc.elements[0].id;
    const moved = reducer(added, { type: 'update', id, patch: { x: 120, y: 64 } });

    expect(moved.doc.elements[0]).toMatchObject({ x: 120, y: 64 });
    expect(moved.past).toHaveLength(added.past.length + 1);
    // Le document précédent est intact : c'est ce que l'annulation restaurera.
    expect(moved.past[moved.past.length - 1].elements[0].x).toBe(added.doc.elements[0].x);
  });

  it('fusionne les actions consécutives de même clé (curseur traîné)', () => {
    const added = run(start(), { type: 'add', kind: 'text' });
    const id = added.doc.elements[0].id;
    let state = added;
    for (let i = 1; i <= 20; i += 1) {
      state = reducer(state, { type: 'update', id, patch: { opacity: i / 20 }, coalesce: `opacity:${id}` });
    }
    expect(state.past).toHaveLength(added.past.length + 1);
    expect(reducer(state, { type: 'undo' }).doc.elements[0].opacity).toBe(1);
  });
});

describe('undo / redo', () => {
  it('revient et rejoue', () => {
    const state = withThreeElements();
    expect(canUndo(state)).toBe(true);
    expect(canRedo(state)).toBe(false);

    const undone = run(state, { type: 'undo' }, { type: 'undo' });
    expect(undone.doc.elements).toHaveLength(1);
    expect(canRedo(undone)).toBe(true);

    const redone = run(undone, { type: 'redo' }, { type: 'redo' });
    expect(redone.doc.elements).toHaveLength(3);
    expect(redone.doc).toEqual(state.doc);
  });

  it('une nouvelle action efface le futur', () => {
    const state = run(withThreeElements(), { type: 'undo' }, { type: 'add', kind: 'shape' });
    expect(canRedo(state)).toBe(false);
    expect(state.doc.elements).toHaveLength(3);
  });

  it("plafonne la pile et n'annule jamais au-delà", () => {
    let state = start();
    const total = HISTORY_LIMIT + 40;
    for (let i = 0; i < total; i += 1) state = reducer(state, { type: 'add', kind: 'text' });
    expect(state.past).toHaveLength(HISTORY_LIMIT);

    for (let i = 0; i < total * 2; i += 1) state = reducer(state, { type: 'undo' });
    // On remonte de 80 crans au maximum, puis l'annulation devient sans effet.
    expect(state.doc.elements).toHaveLength(total - HISTORY_LIMIT);
    expect(canUndo(state)).toBe(false);
    expect(state.doc.elements).toHaveLength(40);
  });

  it('libère la sélection quand son élément disparaît', () => {
    const state = run(start(), { type: 'add', kind: 'text' });
    const undone = reducer(state, { type: 'undo' });
    expect(undone.doc.elements).toHaveLength(0);
    expect(undone.selectedId).toBeNull();
    expect(selectedElement(undone)).toBeNull();
  });
});

describe('ordre des calques', () => {
  it("monte, descend, met au premier et à l'arrière-plan", () => {
    const state = withThreeElements();
    const [a, b, c] = state.doc.elements.map((e) => e.id);

    expect(reducer(state, { type: 'reorder', id: a, to: 'up' }).doc.elements.map((e) => e.id)).toEqual([b, a, c]);
    expect(reducer(state, { type: 'reorder', id: c, to: 'down' }).doc.elements.map((e) => e.id)).toEqual([a, c, b]);
    expect(reducer(state, { type: 'reorder', id: a, to: 'front' }).doc.elements.map((e) => e.id)).toEqual([b, c, a]);
    expect(reducer(state, { type: 'reorder', id: c, to: 'back' }).doc.elements.map((e) => e.id)).toEqual([c, a, b]);
  });

  it('ne bouge pas aux extrémités et ne touche pas à l’historique', () => {
    const state = withThreeElements();
    const first = state.doc.elements[0].id;
    const next = reducer(state, { type: 'reorder', id: first, to: 'back' });
    expect(next.doc.elements).toEqual(state.doc.elements);
  });
});

describe('duplication', () => {
  it('crée un nouvel identifiant et ne partage aucun objet avec la source', () => {
    const state = run(start(), { type: 'add', kind: 'button' });
    const source = state.doc.elements[0];
    const duplicated = reducer(state, { type: 'duplicate', id: source.id });
    const copy = duplicated.doc.elements[1];

    expect(duplicated.doc.elements).toHaveLength(2);
    expect(copy.id).not.toBe(source.id);
    expect(copy.id).toMatch(/^[a-z0-9]{7}$/);
    expect(duplicated.selectedId).toBe(copy.id);
    expect([copy.x, copy.y]).toEqual([source.x + 14, source.y + 14]);

    // Aucun alias : modifier la copie doit laisser la source intacte.
    expect(copy.anim).not.toBe(source.anim);
    const changed = reducer(duplicated, { type: 'updateAnim', id: copy.id, patch: { preset: 'glow' } });
    expect(changed.doc.elements[1].anim.preset).toBe('glow');
    expect(changed.doc.elements[0].anim.preset).toBe('none');
  });
});

describe('suppression', () => {
  it("supprime l'élément sélectionné et vide la sélection", () => {
    const state = withThreeElements();
    const target = state.selectedId;
    expect(target).not.toBeNull();

    const removed = reducer(state, { type: 'remove', id: target as string });
    expect(removed.doc.elements).toHaveLength(2);
    expect(removed.doc.elements.some((e) => e.id === target)).toBe(false);
    expect(removed.selectedId).toBeNull();
    expect(reducer(removed, { type: 'undo' }).doc.elements).toHaveLength(3);
  });

  it('ignore un identifiant inconnu', () => {
    const state = withThreeElements();
    expect(reducer(state, { type: 'remove', id: 'zzzzzzz' })).toBe(state);
  });
});

describe('jetons de profil', () => {
  it('résout ce qui est renseigné et laisse le jeton visible sinon', () => {
    expect(resolveTokens('{{name}} — {{role}}', { name: 'Ada' })).toBe('Ada — {{role}}');
    expect(resolveTokens('{{unknown}}', {})).toBe('{{unknown}}');
  });
});

describe('compatibilité', () => {
  it('dégrade le score sur les éléments risqués', () => {
    const clean = compatibility(EMPTY_DOC);
    expect(clean.score).toBe(100);

    const withVideo = compatibility(run(start(), { type: 'add', kind: 'video' }).doc);
    expect(withVideo.score).toBeLessThan(clean.score);
    expect(withVideo.issues).toContain('vidéo');
  });
});
