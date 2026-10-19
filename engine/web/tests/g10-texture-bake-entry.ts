// Browser entry for the texture bake test: exposes the baker to the page.
import { Baker } from '../../../src/engine/texture/bake';

(window as unknown as { Baker: typeof Baker }).Baker = Baker;
