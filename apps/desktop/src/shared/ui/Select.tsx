import * as SelectPrimitive from '@radix-ui/react-select';
import { Check, ChevronDown, ChevronUp } from 'lucide-react';
import {
  Children,
  isValidElement,
  useRef,
  useState,
  type ButtonHTMLAttributes,
  type ReactNode,
} from 'react';
import './select.css';

type SelectProps = Omit<
  ButtonHTMLAttributes<HTMLButtonElement>,
  'value' | 'defaultValue' | 'onChange' | 'children'
> & {
  value: string | number;
  onValueChange: (value: string) => void;
  children: ReactNode;
  required?: boolean;
};

export function Select({
  value,
  onValueChange,
  children,
  required,
  disabled,
  name,
  className = '',
  ...props
}: SelectProps) {
  const trigger = useRef<HTMLButtonElement>(null);
  const [invalid, setInvalid] = useState(false);
  const emptyOption = Children.toArray(children).find(
    (child) => isValidElement<{ value: string | number }>(child) && child.props.value === '',
  );
  const placeholder = isValidElement<{ children: ReactNode }>(emptyOption)
    ? emptyOption.props.children
    : undefined;
  return (
    <span
      className="select-root"
      onInvalidCapture={(event) => {
        // Native form validation still blocks submission; focus the visible control.
        event.preventDefault();
        setInvalid(true);
        trigger.current?.focus();
      }}
    >
      <SelectPrimitive.Root
        value={String(value)}
        onValueChange={(next) => {
          setInvalid(false);
          onValueChange(next);
        }}
        required={required}
        disabled={disabled}
        name={name}
      >
        <SelectPrimitive.Trigger
          {...props}
          ref={trigger}
          className={`select-control ${className}`}
          aria-invalid={invalid || props['aria-invalid']}
        >
          <SelectPrimitive.Value placeholder={placeholder} />
          <SelectPrimitive.Icon asChild>
            <ChevronDown className="select-chevron" size={14} />
          </SelectPrimitive.Icon>
        </SelectPrimitive.Trigger>
        <SelectPrimitive.Portal>
          <SelectPrimitive.Content
            className="select-menu"
            aria-label={props['aria-label']}
            position="popper"
            sideOffset={5}
            collisionPadding={8}
          >
            <SelectPrimitive.ScrollUpButton className="select-scroll">
              <ChevronUp size={14} />
            </SelectPrimitive.ScrollUpButton>
            <SelectPrimitive.Viewport className="select-options">
              {children}
            </SelectPrimitive.Viewport>
            <SelectPrimitive.ScrollDownButton className="select-scroll">
              <ChevronDown size={14} />
            </SelectPrimitive.ScrollDownButton>
          </SelectPrimitive.Content>
        </SelectPrimitive.Portal>
      </SelectPrimitive.Root>
    </span>
  );
}
export function SelectItem({
  value,
  children,
  disabled,
}: {
  value: string | number;
  children: ReactNode;
  disabled?: boolean;
}) {
  return (
    <SelectPrimitive.Item className="select-option" value={String(value)} disabled={disabled}>
      <SelectPrimitive.ItemText>{children}</SelectPrimitive.ItemText>
      <SelectPrimitive.ItemIndicator className="select-indicator">
        <Check size={14} />
      </SelectPrimitive.ItemIndicator>
    </SelectPrimitive.Item>
  );
}
