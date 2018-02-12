import React, { Component } from "react";
import logo from "./logo.svg";
import "./App.css";
import { Button } from "reactstrap";

const statServer = "http://127.0.0.1:8080/mock.json";

class App extends Component {
  constructor(props) {
    super(props);
    this.state = { nodes: [] };
  }

  componentDidMount() {
    this.timerID = setInterval(() => this.updateNodes(), 1000);
  }

  componentWillUnmount() {
    clearInterval(this.timerID);
  }

  async updateNodes() {
    const rand = Math.random();
    console.log("req", rand);
    const nodes = await (await fetch(statServer)).json();
    console.log("res", rand, nodes);
    this.setState({ nodes });
  }

  render() {
    return (
      <div className="App">
        <header className="App-header">
          <img src={logo} className="App-logo" alt="logo" />
          <h1 className="App-title">Welcome to React</h1>
        </header>
        <p className="App-intro">
          To get started, edit <code>src/App.js</code> and save to reload.
        </p>
        <Button color="danger">
          <h2>It is {JSON.stringify(this.state.nodes)}.</h2>
        </Button>
      </div>
    );
  }
}

export default App;
