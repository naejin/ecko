// Expected: exit 1, check=unused-imports
import { useState, useEffect } from 'react';
import axios from 'axios';

const x = 1;
export default x;
