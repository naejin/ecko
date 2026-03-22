// Expected: exit 0
import React, { useState, useCallback } from 'react';

interface Props {
    label: string;
    onClick: () => void;
}

const Button: React.FC<Props> = ({ label, onClick }) => {
    const [pressed, setPressed] = useState(false);

    const handleClick = useCallback(() => {
        setPressed(true);
        onClick();
    }, [onClick]);

    return <button onClick={handleClick}>{pressed ? 'Clicked' : label}</button>;
};

export default Button;
