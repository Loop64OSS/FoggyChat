import * as tagsInput from '@zag-js/tags-input';
import type { Snippet } from 'svelte';
export interface TagsInputProps extends Omit<tagsInput.Props, 'id'> {
    /** Set the add tag input placeholder. */
    placeholder?: string;
    /** Set base classes for the root. */
    base?: string;
    /** Set gap classes for the root. */
    gap?: string;
    /** Set padding classes for the root. */
    padding?: string;
    /** Provide arbitrary classes to the root. */
    classes?: string;
    /** Set base classes for the add tag input. */
    inputBase?: string;
    /** Provide arbitrary classes to the add tag input. */
    inputClasses?: string;
    /** Set base classes for the tag list. */
    tagListBase?: string;
    /** Provide arbitrary classes to the tag list. */
    tagListClasses?: string;
    /** Set base classes for each tag. */
    tagBase?: string;
    /** Set background classes for each tag. */
    tagBackground?: string;
    /** Provide arbitrary classes to each tag. */
    tagClasses?: string;
    /** Set base classes for the edit tag input. */
    tagEditInputBase?: string;
    /** Provide arbitrary classes to the edit tag input. */
    tagEditInputClasses?: string;
    /** Set base classes for the delete button. */
    buttonDeleteBase?: string;
    /** Provide arbitrary classes to the delete button. */
    buttonDeleteClasses?: string;
    /** Set the component disabled state. */
    stateDisabled?: string;
    /** The delete button label snippet. */
    buttonDelete?: Snippet;
}
