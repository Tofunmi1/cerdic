// Where the game's UI buttons are on screen (in screen heights), as the last UI frame said. GameUi writes it when it draws a frame, GameInput
// reads it to turn a click into the press `ui:ID`; neither decides what a button is.
import type { ButtonBox } from './actionMap';

export const uiButtons: { current: ButtonBox[] } = { current: [] };
