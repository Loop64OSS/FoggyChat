var _a;
import { createContext } from '../../internal/create-context.js';
// @ts-expect-error - Defaults for context don't make sense, `createContext` should just throw TBH
export var setAccordionContext = (_a = createContext(), _a[0]), getAccordionContext = _a[1], key = _a[2];
