import * as rating from '@zag-js/rating-group';
import type { Snippet } from 'svelte';
export interface RatingProps extends Omit<rating.Props, 'id'> {
    /** Set root base classes */
    base?: string;
    /** Set root gap classes */
    gap?: string;
    /** Set root arbitrary classes */
    classes?: string;
    /** Set control base classes */
    controlBase?: string;
    /** Set control gap classes */
    controlGap?: string;
    /** Set control arbitrary classes */
    controlClasses?: string;
    /** Set label base classes */
    labelBase?: string;
    /** Set label arbitrary classes */
    labelClasses?: string;
    /** Set item base classes */
    itemBase?: string;
    /** Set item arbitrary classes */
    itemClasses?: string;
    /** Set item read-only state classes */
    stateReadOnly?: string;
    /** Set item disabled state classes */
    stateDisabled?: string;
    /** Set the empty icon snippet */
    iconEmpty?: Snippet;
    /** Set the half icon snippet */
    iconHalf?: Snippet;
    /** Set the full icon snippet */
    iconFull?: Snippet;
    /** Set the label snippet */
    label?: Snippet;
}
